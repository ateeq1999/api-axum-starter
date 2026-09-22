use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const DEFAULT_MAX_ATTEMPTS: i32 = 5;

#[derive(Debug, Clone, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub payload: String,
    pub attempts: i32,
    pub max_attempts: i32,
}

#[derive(Clone)]
pub struct JobsRepository {
    db: PgPool,
}

impl JobsRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn enqueue(
        &self,
        kind: &str,
        payload: &str,
        run_at: DateTime<Utc>,
    ) -> sqlx::Result<()> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO jobs (id, kind, payload, run_at, max_attempts, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(kind)
        .bind(payload)
        .bind(run_at)
        .bind(DEFAULT_MAX_ATTEMPTS)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Enqueues one `kind` job for right now, unless one is already pending or running.
    /// Used at startup so a fresh database gets its first recurring job scheduled exactly once.
    pub async fn ensure_scheduled(&self, kind: &str) -> sqlx::Result<()> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO jobs (id, kind, payload, run_at, max_attempts, created_at, updated_at)
             SELECT $1, $2, '{}', $3, $4, $3, $3
             WHERE NOT EXISTS (
                 SELECT 1 FROM jobs WHERE kind = $2 AND status IN ('pending', 'running')
             )",
        )
        .bind(Uuid::new_v4())
        .bind(kind)
        .bind(now)
        .bind(DEFAULT_MAX_ATTEMPTS)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Atomically claims up to `limit` due jobs. `FOR UPDATE SKIP LOCKED` means two workers
    /// (or two polls racing each other, even across processes sharing this database) can never
    /// both claim the same job, and neither blocks waiting on the other.
    pub async fn claim_batch(&self, limit: i64) -> sqlx::Result<Vec<Job>> {
        let now = Utc::now();
        sqlx::query_as::<_, Job>(
            "WITH claimed AS (
                 SELECT id FROM jobs
                 WHERE status = 'pending' AND run_at <= $1
                 ORDER BY run_at
                 LIMIT $2
                 FOR UPDATE SKIP LOCKED
             )
             UPDATE jobs SET status = 'running', locked_at = $1, updated_at = $1
             WHERE id IN (SELECT id FROM claimed)
             RETURNING id, kind, payload, attempts, max_attempts",
        )
        .bind(now)
        .bind(limit)
        .fetch_all(&self.db)
        .await
    }

    pub async fn mark_succeeded(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE jobs SET status = 'succeeded', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Reschedules with backoff, or marks `dead` (kept, not deleted, so it stays inspectable)
    /// once `attempts` reaches `max_attempts`.
    pub async fn reschedule_or_kill(
        &self,
        job: &Job,
        error: &str,
        backoff: chrono::Duration,
    ) -> sqlx::Result<bool> {
        let now = Utc::now();
        let attempts = job.attempts + 1;
        let dead = attempts >= job.max_attempts;
        let status = if dead { "dead" } else { "pending" };
        let run_at = now + backoff;
        sqlx::query(
            "UPDATE jobs SET status = $1, attempts = $2, run_at = $3, locked_at = NULL,
                              last_error = $4, updated_at = $5
             WHERE id = $6",
        )
        .bind(status)
        .bind(attempts)
        .bind(run_at)
        .bind(error)
        .bind(now)
        .bind(job.id)
        .execute(&self.db)
        .await?;
        Ok(dead)
    }
}
