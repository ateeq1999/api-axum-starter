//! General-purpose object storage: put, get and delete bytes by key, on local disk or in S3.
//!
//! Features (profile photos, user media, ...) share one `ObjectStorage` handle and keep their
//! objects apart with a key prefix (`avatars/`, `media/`). Nothing here knows what an object
//! is: validation of *what* may be stored belongs to the feature, validation of the *key*
//! (so a caller bug can never escape the storage root) belongs here.

use std::{io::ErrorKind, path::PathBuf, sync::Arc};

use aws_sdk_s3::{error::DisplayErrorContext, primitives::ByteStream, types::ObjectCannedAcl};

use crate::{
    common::error::AppResult,
    config::{S3StorageConfig, StorageConfig},
};

/// Cheap to clone: the backend sits behind an `Arc`, so this can live in `AppState`.
#[derive(Clone)]
pub struct ObjectStorage {
    backend: Arc<StorageBackend>,
}

enum StorageBackend {
    LocalDisk {
        root_directory: PathBuf,
    },
    S3 {
        client: aws_sdk_s3::Client,
        bucket: String,
    },
}

impl ObjectStorage {
    /// S3 when `S3_BUCKET` is configured, otherwise local disk under `UPLOAD_DIR`.
    pub async fn from_config(config: &StorageConfig) -> anyhow::Result<Self> {
        let backend = match &config.s3 {
            Some(s3_config) => connect_s3(s3_config).await?,
            None => {
                tokio::fs::create_dir_all(&config.upload_dir).await?;
                StorageBackend::LocalDisk {
                    root_directory: config.upload_dir.clone(),
                }
            }
        };
        Ok(Self {
            backend: Arc::new(backend),
        })
    }

    /// Keys are `/`-separated segments of `[A-Za-z0-9._-]`, none empty, `.` or `..`. Callers
    /// build keys from a fixed prefix plus a generated id, so this is a backstop, not the
    /// primary defence.
    pub fn is_valid_key(key: &str) -> bool {
        !key.is_empty()
            && key.split('/').all(|segment| {
                !segment.is_empty()
                    && segment != "."
                    && segment != ".."
                    && segment
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            })
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>, content_type: &str) -> AppResult<()> {
        if !Self::is_valid_key(key) {
            return Err(anyhow::anyhow!("invalid storage key `{key}`").into());
        }
        match &*self.backend {
            StorageBackend::LocalDisk { root_directory } => {
                let destination = root_directory.join(key);
                if let Some(parent) = destination.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| anyhow::anyhow!("creating storage directory: {e}"))?;
                }
                // Write then rename, so a reader never sees a half-written file.
                let temporary = destination.with_extension("tmp");
                tokio::fs::write(&temporary, bytes)
                    .await
                    .map_err(|e| anyhow::anyhow!("writing object: {e}"))?;
                tokio::fs::rename(&temporary, &destination)
                    .await
                    .map_err(|e| anyhow::anyhow!("storing object: {e}"))?;
            }
            StorageBackend::S3 { client, bucket } => {
                // A single PUT is atomic: readers see either the whole object or none of it.
                client
                    .put_object()
                    .bucket(bucket)
                    .key(key)
                    .content_type(content_type)
                    .acl(ObjectCannedAcl::Private)
                    .body(ByteStream::from(bytes))
                    .send()
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!("uploading object to S3: {}", DisplayErrorContext(&e))
                    })?;
            }
        }
        Ok(())
    }

    /// `Ok(None)` when no object has this key.
    pub async fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        if !Self::is_valid_key(key) {
            return Ok(None);
        }
        match &*self.backend {
            StorageBackend::LocalDisk { root_directory } => {
                match tokio::fs::read(root_directory.join(key)).await {
                    Ok(bytes) => Ok(Some(bytes)),
                    Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(anyhow::anyhow!("reading object: {e}").into()),
                }
            }
            StorageBackend::S3 { client, bucket } => {
                let response = match client.get_object().bucket(bucket).key(key).send().await {
                    Ok(response) => response,
                    Err(error)
                        if error
                            .as_service_error()
                            .is_some_and(|service_error| service_error.is_no_such_key()) =>
                    {
                        return Ok(None);
                    }
                    Err(error) => {
                        return Err(anyhow::anyhow!(
                            "reading object from S3: {}",
                            DisplayErrorContext(&error)
                        )
                        .into());
                    }
                };
                let body = response
                    .body
                    .collect()
                    .await
                    .map_err(|e| anyhow::anyhow!("reading object body from S3: {e}"))?;
                Ok(Some(body.to_vec()))
            }
        }
    }

    /// Best effort: an orphaned object is harmless, so failures are only logged. Deleting a
    /// missing key is not an error.
    pub async fn delete(&self, key: &str) {
        if !Self::is_valid_key(key) {
            return;
        }
        match &*self.backend {
            StorageBackend::LocalDisk { root_directory } => {
                if let Err(e) = tokio::fs::remove_file(root_directory.join(key)).await
                    && e.kind() != ErrorKind::NotFound
                {
                    tracing::warn!(error = %e, key, "could not delete stored object");
                }
            }
            StorageBackend::S3 { client, bucket } => {
                if let Err(error) = client.delete_object().bucket(bucket).key(key).send().await {
                    tracing::warn!(
                        error = %DisplayErrorContext(&error),
                        key,
                        "could not delete stored object from S3"
                    );
                }
            }
        }
    }
}

/// Builds the client from the standard AWS environment (credentials, region, and
/// `AWS_ENDPOINT_URL` for emulators) and checks the bucket is reachable, so a wrong bucket
/// name or missing credentials fails at startup rather than on the first upload.
async fn connect_s3(s3_config: &S3StorageConfig) -> anyhow::Result<StorageBackend> {
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let mut client_config = aws_sdk_s3::config::Builder::from(&sdk_config);
    client_config.set_force_path_style(Some(s3_config.force_path_style));
    // Explicit either way: `None` (blank `AWS_ENDPOINT_URL`) must mean real AWS, not the
    // invalid empty endpoint the SDK would otherwise read from the environment.
    client_config.set_endpoint_url(s3_config.endpoint_url.clone());
    let client = aws_sdk_s3::Client::from_conf(client_config.build());

    client
        .head_bucket()
        .bucket(&s3_config.bucket)
        .send()
        .await
        .map_err(|error| {
            anyhow::anyhow!(
                "cannot reach S3 bucket `{}` (check S3_BUCKET, AWS credentials, region and \
                 AWS_ENDPOINT_URL): {}",
                s3_config.bucket,
                DisplayErrorContext(&error)
            )
        })?;

    Ok(StorageBackend::S3 {
        client,
        bucket: s3_config.bucket.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_safe_keys_are_valid() {
        for good in ["a", "avatars/0123.jpg", "media/ab-cd_ef.png", "x/y/z"] {
            assert!(ObjectStorage::is_valid_key(good), "{good}");
        }
        for bad in [
            "",
            "/",
            "/abs",
            "a//b",
            "a/",
            "../secret",
            "a/../b",
            "./a",
            "a\\b",
            "a b",
            "a?b",
            "ü",
        ] {
            assert!(!ObjectStorage::is_valid_key(bad), "{bad}");
        }
    }
}
