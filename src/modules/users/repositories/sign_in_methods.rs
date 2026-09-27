//! How many ways an account can sign in.

use uuid::Uuid;

use super::UsersRepository;
use crate::common::error::AppResult;

impl UsersRepository {
    /// How many ways the user can currently sign in: password, linked OAuth accounts, passkeys.
    /// (Deliberately reads the other features' tables: it is the one place that must know the total.)
    pub async fn sign_in_method_count(&self, id: Uuid) -> AppResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT (SELECT CASE WHEN password_set THEN 1 ELSE 0 END FROM users WHERE id = $1)
                  + (SELECT COUNT(*) FROM oauth_identities WHERE user_id = $1)
                  + (SELECT COUNT(*) FROM passkeys WHERE user_id = $1)",
        )
        .bind(id)
        .fetch_one(&self.db)
        .await?)
    }
}
