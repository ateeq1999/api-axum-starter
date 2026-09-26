use uuid::Uuid;

use super::error::AvatarError;
use crate::{common::error::AppResult, infra::storage::ObjectStorage};

const KEY_PREFIX: &str = "avatars/";

/// Profile photos, kept under the `avatars/` prefix of the shared [`ObjectStorage`].
///
/// File names are generated here (`<uuid>.jpg`) and looked up with strict validation, so a
/// request can never name a key outside the avatars area.
#[derive(Clone)]
pub struct AvatarStorage {
    objects: ObjectStorage,
}

impl AvatarStorage {
    pub fn new(objects: ObjectStorage) -> Self {
        Self { objects }
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
        self.objects
            .put(&format!("{KEY_PREFIX}{name}"), bytes.to_vec(), "image/jpeg")
            .await
    }

    pub async fn read(&self, name: &str) -> AppResult<Vec<u8>> {
        if !Self::is_valid_name(name) {
            return Err(AvatarError::NotFound.into());
        }
        self.objects
            .get(&format!("{KEY_PREFIX}{name}"))
            .await?
            .ok_or_else(|| AvatarError::NotFound.into())
    }

    /// Best effort: an orphaned file is harmless, so failures are only logged.
    pub async fn remove(&self, name: &str) {
        if Self::is_valid_name(name) {
            self.objects.delete(&format!("{KEY_PREFIX}{name}")).await;
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
