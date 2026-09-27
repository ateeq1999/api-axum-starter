//! How many ways an account can sign in, for the oauth and passkeys features.

use super::UsersService;
use crate::common::error::AppResult;
use uuid::Uuid;

impl UsersService {
    pub async fn sign_in_method_count(&self, id: Uuid) -> AppResult<i64> {
        self.repo.sign_in_method_count(id).await
    }
}
