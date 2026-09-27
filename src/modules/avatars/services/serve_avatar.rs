//! Reading a stored photo for the public image URL.

use super::AvatarService;
use crate::common::error::AppResult;

impl AvatarService {
    pub async fn read(&self, file: &str) -> AppResult<Vec<u8>> {
        self.storage.read(file).await
    }
}
