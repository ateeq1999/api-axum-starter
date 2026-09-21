use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::User;
use crate::error::{AppError, AppResult};

pub async fn create(db: &SqlitePool, email: &str, password_hash: &str) -> AppResult<User> {
    let result = sqlx::query_as::<_, User>(
        "INSERT INTO users (id, email, password_hash, created_at)
         VALUES (?, ?, ?, ?)
         RETURNING id, email, password_hash, created_at",
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(password_hash)
    .bind(Utc::now())
    .fetch_one(db)
    .await;

    match result {
        Ok(user) => Ok(user),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(AppError::Conflict("email is already registered".into()))
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn find_by_email(db: &SqlitePool, email: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, created_at FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(db)
    .await?)
}

pub async fn find_by_id(db: &SqlitePool, id: Uuid) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, created_at FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}
