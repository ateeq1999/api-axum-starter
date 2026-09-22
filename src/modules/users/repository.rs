use chrono::Utc;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::{
    dto::{SortOrder, UserSort},
    entity::User,
    error::UsersError,
};
use crate::common::{
    error::{AppError, AppResult},
    security::Role,
};

macro_rules! columns {
    () => {
        "id, email, password_hash, display_name, role, is_active, email_verified_at, created_at, updated_at, deleted_at, avatar_key, password_set"
    };
}

pub struct NewUser<'a> {
    pub email: &'a str,
    pub password_hash: &'a str,
    pub display_name: Option<&'a str>,
    pub role: Role,
    pub email_verified: bool,
    /// `false` for accounts created through OAuth that have no usable password.
    pub password_set: bool,
}

#[derive(Default)]
pub struct UserPatch<'a> {
    pub display_name: Option<&'a str>,
    pub role: Option<Role>,
    pub is_active: Option<bool>,
}

pub struct UserFilter<'a> {
    pub search: Option<&'a str>,
    pub sort: UserSort,
    pub order: SortOrder,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone)]
pub struct UsersRepository {
    db: PgPool,
}

impl UsersRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

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

    pub async fn list(&self, filter: &UserFilter<'_>) -> AppResult<(Vec<User>, u64)> {
        let pattern = filter.search.map(|q| format!("%{}%", escape_like(q)));

        let mut count =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL");
        push_search(&mut count, pattern.as_deref());
        let total: i64 = count.build_query_scalar().fetch_one(&self.db).await?;

        let column = match filter.sort {
            UserSort::CreatedAt => "created_at",
            UserSort::Email => "email",
        };
        let direction = match filter.order {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };

        let mut select = QueryBuilder::<Postgres>::new(concat!(
            "SELECT ",
            columns!(),
            " FROM users WHERE deleted_at IS NULL"
        ));
        push_search(&mut select, pattern.as_deref());
        // `column` and `direction` come from closed enums, never from user text.
        select.push(format!(
            " ORDER BY {column} {direction}, id {direction} LIMIT "
        ));
        select.push_bind(filter.limit);
        select.push(" OFFSET ");
        select.push_bind(filter.offset);
        let users = select.build_query_as::<User>().fetch_all(&self.db).await?;

        Ok((users, total.max(0) as u64))
    }

    pub async fn update(&self, id: Uuid, patch: UserPatch<'_>) -> AppResult<Option<User>> {
        let mut qb = QueryBuilder::<Postgres>::new("UPDATE users SET updated_at = ");
        qb.push_bind(Utc::now());
        if let Some(name) = patch.display_name {
            qb.push(", display_name = ").push_bind(name);
        }
        if let Some(role) = patch.role {
            qb.push(", role = ").push_bind(role);
        }
        if let Some(active) = patch.is_active {
            qb.push(", is_active = ").push_bind(active);
        }
        qb.push(" WHERE id = ").push_bind(id);
        qb.push(concat!(" AND deleted_at IS NULL RETURNING ", columns!()));
        Ok(qb.build_query_as::<User>().fetch_optional(&self.db).await?)
    }

    pub async fn set_password_hash(&self, id: Uuid, password_hash: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET password_hash = $1, password_set = TRUE, updated_at = $2 WHERE id = $3 AND deleted_at IS NULL")
            .bind(password_hash)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn mark_email_verified(&self, id: Uuid) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE users SET email_verified_at = COALESCE(email_verified_at, $1), updated_at = $2
             WHERE id = $3 AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Swaps the address and marks it verified (the caller proved control of it).
    pub async fn change_email(&self, id: Uuid, new_email: &str) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE users SET email = $1, email_verified_at = $2, updated_at = $3
             WHERE id = $4 AND deleted_at IS NULL",
        )
        .bind(new_email)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.db)
        .await
        .map_err(map_write_error)?;
        Ok(())
    }

    /// Soft delete. The email is replaced so the address can be registered again.
    pub async fn soft_delete(&self, id: Uuid, released_email: &str) -> AppResult<bool> {
        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE users SET deleted_at = $1, updated_at = $2, is_active = FALSE, email = $3
             WHERE id = $4 AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(released_email)
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Sets (or clears) the profile photo. Returns the previous file name so the caller can
    /// delete the old file.
    pub async fn set_avatar_key(
        &self,
        id: Uuid,
        avatar_key: Option<&str>,
    ) -> AppResult<Option<Option<String>>> {
        let mut tx = self.db.begin().await?;
        let previous: Option<Option<String>> =
            sqlx::query_scalar("SELECT avatar_key FROM users WHERE id = $1 AND deleted_at IS NULL")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if previous.is_none() {
            return Ok(None);
        }
        sqlx::query("UPDATE users SET avatar_key = $1, updated_at = $2 WHERE id = $3")
            .bind(avatar_key)
            .bind(Utc::now())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(previous)
    }

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

    pub async fn count_active_admins(&self) -> AppResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = TRUE AND deleted_at IS NULL",
        )
        .fetch_one(&self.db)
        .await?)
    }
}

fn push_search(qb: &mut QueryBuilder<Postgres>, pattern: Option<&str>) {
    if let Some(pattern) = pattern {
        qb.push(" AND (email LIKE ")
            .push_bind(pattern.to_string())
            .push(" ESCAPE '\\' OR display_name LIKE ")
            .push_bind(pattern.to_string())
            .push(" ESCAPE '\\')");
    }
}

fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn map_write_error(error: sqlx::Error) -> AppError {
    match error {
        sqlx::Error::Database(e) if e.is_unique_violation() => UsersError::EmailTaken.into(),
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(escape_like("50%_off\\"), "50\\%\\_off\\\\");
    }
}
