//! Changes an administrator makes to other accounts: role, active status, deletion. Each one
//! that could remove an admin runs under the advisory lock (see `ADMIN_GUARD_KEY`).

use chrono::Utc;
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use super::{ADMIN_GUARD_KEY, UserPatch, UsersRepository};
use crate::{
    common::error::AppResult,
    modules::users::{entity::User, error::UsersError},
};

impl UsersRepository {
    /// `guard_last_admin`: when true, the update is rejected (inside the same transaction as the
    /// count check, so no concurrent request can race it) if it would leave zero active admins.
    /// Callers pass `true` only when the patch could plausibly remove an admin (demote or
    /// deactivate); anything else (e.g. a display-name-only change) skips the lock entirely.
    pub async fn update(
        &self,
        id: Uuid,
        patch: UserPatch<'_>,
        guard_last_admin: bool,
    ) -> AppResult<Option<User>> {
        let mut tx = self.db.begin().await?;
        if guard_last_admin {
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(ADMIN_GUARD_KEY)
                .execute(&mut *tx)
                .await?;
            let others: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM users
                 WHERE role = 'admin' AND is_active = TRUE AND deleted_at IS NULL AND id <> $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if others == 0 {
                return Err(UsersError::CannotRemoveLastAdmin.into());
            }
        }

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
        let updated = qb.build_query_as::<User>().fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Soft delete. The row keeps its email (uniqueness only applies to accounts that are not
    /// deleted, so the address can be registered again). See [`Self::update`] for what
    /// `guard_last_admin` does.
    pub async fn soft_delete(&self, id: Uuid, guard_last_admin: bool) -> AppResult<bool> {
        let mut tx = self.db.begin().await?;
        if guard_last_admin {
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(ADMIN_GUARD_KEY)
                .execute(&mut *tx)
                .await?;
            let others: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM users
                 WHERE role = 'admin' AND is_active = TRUE AND deleted_at IS NULL AND id <> $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if others == 0 {
                return Err(UsersError::CannotRemoveLastAdmin.into());
            }
        }

        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE users SET deleted_at = $1, updated_at = $2, is_active = FALSE
             WHERE id = $3 AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn count_active_admins(&self) -> AppResult<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = TRUE AND deleted_at IS NULL",
        )
        .fetch_one(&self.db)
        .await?)
    }
}
