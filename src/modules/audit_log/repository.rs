use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::entity::AuditLogEntry;
use crate::common::error::AppResult;

macro_rules! columns {
    () => {
        "id, actor_user_id, action, target_user_id, details, created_at"
    };
}

#[derive(Clone)]
pub struct AuditLogRepository {
    db: PgPool,
}

impl AuditLogRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn record(
        &self,
        actor_user_id: Uuid,
        action: &str,
        target_user_id: Option<Uuid>,
        details_json: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO audit_log (id, actor_user_id, action, target_user_id, details, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(actor_user_id)
        .bind(action)
        .bind(target_user_id)
        .bind(details_json)
        .bind(Utc::now())
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Newest first. `limit`/`offset` come from an already-validated [`crate::common::dto::Pagination`].
    pub async fn list(&self, limit: i64, offset: i64) -> AppResult<(Vec<AuditLogEntry>, u64)> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.db)
            .await?;
        let entries = sqlx::query_as::<_, AuditLogEntry>(concat!(
            "SELECT ",
            columns!(),
            " FROM audit_log ORDER BY created_at DESC, id DESC LIMIT $1 OFFSET $2"
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;
        Ok((entries, total.max(0) as u64))
    }
}
