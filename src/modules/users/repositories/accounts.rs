//! Creating accounts and looking them up.

use chrono::Utc;
use uuid::Uuid;

use super::{NewUser, UsersRepository, map_write_error};
use crate::{common::error::AppResult, modules::users::entity::User};

impl UsersRepository {
    pub async fn create(&self, new: NewUser<'_>) -> AppResult<User> {
        let now = Utc::now();
        sqlx::query_as::<_, User>(concat!(
            "INSERT INTO users (id, email, password_hash, display_name, role, email_verified_at, created_at, password_set)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING ",
            columns!()
        ))
            .bind(Uuid::new_v4())
            .bind(new.email)
            .bind(new.password_hash)
            .bind(new.display_name)
            .bind(new.role)
            .bind(new.email_verified.then_some(now))
            .bind(now)
            .bind(new.password_set)
            .fetch_one(&self.db)
            .await
            .map_err(map_write_error)
    }

    pub async fn find_by_email(&self, email: &str) -> AppResult<Option<User>> {
        Ok(sqlx::query_as::<_, User>(concat!(
            "SELECT ",
            columns!(),
            " FROM users WHERE email = $1 AND deleted_at IS NULL"
        ))
        .bind(email)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<User>> {
        Ok(sqlx::query_as::<_, User>(concat!(
            "SELECT ",
            columns!(),
            " FROM users WHERE id = $1 AND deleted_at IS NULL"
        ))
        .bind(id)
        .fetch_optional(&self.db)
        .await?)
    }
}
