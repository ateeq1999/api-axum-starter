use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
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
        "id, email, password_hash, display_name, role, is_active, email_verified_at, created_at, updated_at, deleted_at"
    };
}

pub struct NewUser<'a> {
    pub email: &'a str,
    pub password_hash: &'a str,
    pub display_name: Option<&'a str>,
    pub role: Role,
    pub email_verified: bool,
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
    db: SqlitePool,
}

impl UsersRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn create(&self, new: NewUser<'_>) -> AppResult<User> {
        let now = Utc::now();
        sqlx::query_as::<_, User>(concat!(
            "INSERT INTO users (id, email, password_hash, display_name, role, email_verified_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
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
            .fetch_one(&self.db)
            .await
            .map_err(map_write_error)
    }

    pub async fn find_by_email(&self, email: &str) -> AppResult<Option<User>> {
        Ok(sqlx::query_as::<_, User>(concat!(
            "SELECT ",
            columns!(),
            " FROM users WHERE email = ? AND deleted_at IS NULL"
        ))
        .bind(email)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<User>> {
        Ok(sqlx::query_as::<_, User>(concat!(
            "SELECT ",
            columns!(),
            " FROM users WHERE id = ? AND deleted_at IS NULL"
        ))
        .bind(id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn list(&self, filter: &UserFilter<'_>) -> AppResult<(Vec<User>, u64)> {
        let pattern = filter.search.map(|q| format!("%{}%", escape_like(q)));

        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL");
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

        let mut select = QueryBuilder::<Sqlite>::new(concat!(
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
        let mut qb = QueryBuilder::<Sqlite>::new("UPDATE users SET updated_at = ");
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
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
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
            "UPDATE users SET email_verified_at = COALESCE(email_verified_at, ?), updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
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
            "UPDATE users SET email = ?, email_verified_at = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
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
            "UPDATE users SET deleted_at = ?, updated_at = ?, is_active = 0, email = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(released_email)
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn count_active_admins(&self) -> AppResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1 AND deleted_at IS NULL",
        )
        .fetch_one(&self.db)
        .await?)
    }
}

fn push_search(qb: &mut QueryBuilder<Sqlite>, pattern: Option<&str>) {
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
