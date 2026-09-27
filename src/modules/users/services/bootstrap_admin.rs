//! The first administrator, created at startup.

use super::UsersService;
use crate::common::{error::AppResult, security::Role};

impl UsersService {
    /// Creates the first administrator when none exists. Safe to call on every startup.
    pub async fn ensure_bootstrap_admin(&self, email: &str, password: String) -> AppResult<()> {
        if self.repo.count_active_admins().await? > 0 {
            return Ok(());
        }
        match self
            .create_with_password(email, password, None, Role::Admin, true)
            .await
        {
            Ok(admin) => {
                tracing::info!(email = %admin.email, "bootstrap administrator created");
                Ok(())
            }
            Err(error) => {
                tracing::warn!(?error, "could not create bootstrap administrator");
                Ok(())
            }
        }
    }
}
