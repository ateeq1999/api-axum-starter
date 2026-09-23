use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::transport::OutgoingMail;

/// Durable record of mail between "rendered" and "delivered" (see `outbound_mail`'s migration
/// comment). Opt-in: only wired up for the real SMTP-backed `MailService` (see
/// `MailService::with_durable_outbox`), never for the in-memory test transport.
#[derive(Clone)]
pub struct OutboxRepository {
    db: PgPool,
}

impl OutboxRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn insert(&self, mail: &OutgoingMail) -> sqlx::Result<Uuid> {
        let id = Uuid::new_v4();
        // `OutgoingMail` always serializes: it is a handful of plain strings.
        let payload = serde_json::to_string(mail).expect("OutgoingMail is always serializable");
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO outbound_mail (id, payload, status, created_at, updated_at)
             VALUES ($1, $2, 'pending', $3, $3)",
        )
        .bind(id)
        .bind(payload)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(id)
    }

    /// Delivery succeeded: nothing left to recover.
    pub async fn remove(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM outbound_mail WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Delivery was permanently given up on (same retry budget the in-memory loop already used
    /// before this record existed). Kept, not deleted, so a real failure stays inspectable —
    /// pruned after 30 days by `infra::jobs::cleanup`, same as the job queue's dead letters.
    pub async fn mark_dead(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE outbound_mail SET status = 'dead', updated_at = $2 WHERE id = $1")
            .bind(id)
            .bind(Utc::now())
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Rows still `pending` at startup: the process crashed (or was killed) between rendering
    /// and delivering them. Unreadable payloads are skipped and logged rather than panicking one
    /// bad row into blocking recovery of every other pending email.
    pub async fn pending(&self) -> sqlx::Result<Vec<(Uuid, OutgoingMail)>> {
        let rows: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT id, payload FROM outbound_mail WHERE status = 'pending'")
                .fetch_all(&self.db)
                .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, payload)| match serde_json::from_str(&payload) {
                Ok(mail) => Some((id, mail)),
                Err(error) => {
                    tracing::error!(%error, mail_id = %id, "unreadable outbound_mail payload, skipping");
                    None
                }
            })
            .collect())
    }
}
