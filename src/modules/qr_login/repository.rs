use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::entity::{QrSession, QrStatus};
use crate::common::error::AppResult;

/// After this many wrong verification codes the session is cancelled.
pub const MAX_CODE_ATTEMPTS: i64 = 3;

macro_rules! columns {
    () => {
        "id, secret_hash, code, status, user_id, requester_ip, requester_agent, attempts, expires_at, created_at"
    };
}

#[derive(Clone)]
pub struct QrRepository {
    db: SqlitePool,
}

impl QrRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn insert(
        &self,
        id: &str,
        secret_hash: &str,
        code: &str,
        requester_ip: Option<&str>,
        requester_agent: Option<&str>,
        expires_at: DateTime<Utc>,
    ) -> AppResult<()> {
        let now = Utc::now();
        // Opportunistic cleanup of long-dead sessions.
        sqlx::query("DELETE FROM qr_sessions WHERE expires_at < ?")
            .bind(now - Duration::days(1))
            .execute(&self.db)
            .await?;
        sqlx::query(
            "INSERT INTO qr_sessions
                 (id, secret_hash, code, status, requester_ip, requester_agent, expires_at, created_at)
             VALUES (?, ?, ?, 'pending', ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(secret_hash)
        .bind(code)
        .bind(requester_ip)
        .bind(requester_agent)
        .bind(expires_at)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn find(&self, id: &str) -> AppResult<Option<QrSession>> {
        Ok(sqlx::query_as::<_, QrSession>(concat!(
            "SELECT ",
            columns!(),
            " FROM qr_sessions WHERE id = ?"
        ))
        .bind(id)
        .fetch_optional(&self.db)
        .await?)
    }

    /// pending -> scanned, atomically. False if someone else got there first, or it expired.
    pub async fn mark_scanned(&self, id: &str, user_id: Uuid) -> AppResult<bool> {
        let result = sqlx::query(
            "UPDATE qr_sessions SET status = 'scanned', user_id = ?
             WHERE id = ? AND status = 'pending' AND expires_at > ?",
        )
        .bind(user_id)
        .bind(id)
        .bind(Utc::now())
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// scanned -> approved/rejected, only by the user who scanned it.
    pub async fn resolve(&self, id: &str, user_id: Uuid, to: QrStatus) -> AppResult<bool> {
        debug_assert!(matches!(to, QrStatus::Approved | QrStatus::Rejected));
        let result = sqlx::query(
            "UPDATE qr_sessions SET status = ?
             WHERE id = ? AND status = 'scanned' AND user_id = ? AND expires_at > ?",
        )
        .bind(to)
        .bind(id)
        .bind(user_id)
        .bind(Utc::now())
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Counts a wrong verification code; cancels the session on the last allowed attempt.
    /// Returns true when the session was cancelled.
    pub async fn record_wrong_code(&self, id: &str) -> AppResult<bool> {
        let status: Option<QrStatus> = sqlx::query_scalar(
            "UPDATE qr_sessions
             SET attempts = attempts + 1,
                 status = CASE WHEN attempts + 1 >= ? THEN 'rejected' ELSE status END
             WHERE id = ? AND status = 'scanned'
             RETURNING status",
        )
        .bind(MAX_CODE_ATTEMPTS)
        .bind(id)
        .fetch_optional(&self.db)
        .await?;
        Ok(status == Some(QrStatus::Rejected))
    }

    /// approved -> consumed, atomically: the access token is handed out exactly once.
    pub async fn consume(&self, id: &str) -> AppResult<Option<Uuid>> {
        Ok(sqlx::query_scalar(
            "UPDATE qr_sessions SET status = 'consumed'
             WHERE id = ? AND status = 'approved' AND expires_at > ?
             RETURNING user_id",
        )
        .bind(id)
        .bind(Utc::now())
        .fetch_optional(&self.db)
        .await?)
    }
}
