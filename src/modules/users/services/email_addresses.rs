//! Email verification and email changes, for the auth feature.

use super::{UsersService, normalize_email};
use crate::common::error::AppResult;
use uuid::Uuid;

impl UsersService {
    pub async fn mark_email_verified(&self, id: Uuid) -> AppResult<()> {
        self.repo.mark_email_verified(id).await
    }

    pub async fn change_email(&self, id: Uuid, new_email: &str) -> AppResult<()> {
        self.repo
            .change_email(id, &normalize_email(new_email))
            .await
    }
}
