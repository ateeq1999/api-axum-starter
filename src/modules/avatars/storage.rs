use std::{io::ErrorKind, path::PathBuf};

use uuid::Uuid;

use super::error::AvatarError;
use crate::common::error::AppResult;

/// Stores avatar files on local disk under `<upload_dir>/avatars`.
///
/// File names are generated here (`<uuid>.jpg`) and looked up with strict validation, so a
/// request can never name a path outside the directory.
#[derive(Clone)]
pub struct AvatarStorage {
    dir: PathBuf,
}

impl AvatarStorage {
    pub async fn new(upload_dir: PathBuf) -> anyhow::Result<Self> {
        let dir = upload_dir.join("avatars");
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self { dir })
    }

    /// A fresh unguessable name. It changes on every upload, which is what lets the browser
    /// cache each file forever.
    pub fn new_name() -> String {
        format!("{}.jpg", Uuid::new_v4().simple())
    }

    pub fn is_valid_name(name: &str) -> bool {
        name.len() == 36
            && name.ends_with(".jpg")
            && name[..32]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }

    pub async fn write(&self, name: &str, bytes: &[u8]) -> AppResult<()> {
        debug_assert!(Self::is_valid_name(name));
        // Write then rename, so a reader never sees a half-written file.
        let temp = self.dir.join(format!("{name}.tmp"));
        tokio::fs::write(&temp, bytes)
            .await
            .map_err(|e| anyhow::anyhow!("writing avatar: {e}"))?;
        tokio::fs::rename(&temp, self.dir.join(name))
            .await
            .map_err(|e| anyhow::anyhow!("storing avatar: {e}"))?;
        Ok(())
    }

    pub async fn read(&self, name: &str) -> AppResult<Vec<u8>> {
        if !Self::is_valid_name(name) {
            return Err(AvatarError::NotFound.into());
        }
        match tokio::fs::read(self.dir.join(name)).await {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == ErrorKind::NotFound => Err(AvatarError::NotFound.into()),
            Err(e) => Err(anyhow::anyhow!("reading avatar: {e}").into()),
        }
    }

    /// Best effort: an orphaned file is harmless, so failures are only logged.
    pub async fn remove(&self, name: &str) {
        if !Self::is_valid_name(name) {
            return;
        }
        if let Err(e) = tokio::fs::remove_file(self.dir.join(name)).await
            && e.kind() != ErrorKind::NotFound
        {
            tracing::warn!(error = %e, file = name, "could not delete old avatar");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_generated_names_are_valid() {
        assert!(AvatarStorage::is_valid_name(&AvatarStorage::new_name()));
        for bad in [
            "",
            "../secret.jpg",
            "..\\secret.jpg",
            "0123456789abcdef0123456789abcdef.png",
            "0123456789ABCDEF0123456789abcdef.jpg",
            "0123456789abcdef0123456789abcde/.jpg",
            "0123456789abcdef0123456789abcdef.jpg/x",
        ] {
            assert!(!AvatarStorage::is_valid_name(bad), "{bad}");
        }
    }
}
