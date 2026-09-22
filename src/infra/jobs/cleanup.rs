//! The recurring "sweep expired rows" job — a cron job with no `pg_cron` needed: on success it
//! re-enqueues itself an hour out (see `worker::dispatch`).
//!
//! Nothing used to sweep these opportunistically-cleaned tables on a schedule; an idle instance
//! never revisited them. A one-hour grace period after `expires_at` is kept before deleting, so a
//! row already being polled (e.g. a QR session showing "expired") is not pulled out from under it.
//!
//! Each cycle also prunes the `jobs` table itself: a re-enqueuing job leaves its finished row
//! behind as history (never updated in place), so without this the table would grow by one row
//! every cycle forever. Succeeded rows are kept a day (enough to see recent runs); dead rows —
//! real failures worth investigating — are kept a month before being dropped.

use chrono::{Duration, Utc};
use sqlx::PgPool;

pub const KIND: &str = "cleanup_expired_rows";
const INTERVAL: Duration = Duration::hours(1);
const SUCCEEDED_JOB_RETENTION: Duration = Duration::days(1);
const DEAD_JOB_RETENTION: Duration = Duration::days(30);

pub async fn run(db: &PgPool) -> Result<(), sqlx::Error> {
    let cutoff = Utc::now() - Duration::hours(1);
    // One static literal per table (rather than a formatted string) so each stays a
    // compile-time-checked-safe `&'static str`, never a dynamically built query.
    for (table, sql) in [
        (
            "auth_tokens",
            "DELETE FROM auth_tokens WHERE expires_at < $1",
        ),
        (
            "oauth_states",
            "DELETE FROM oauth_states WHERE expires_at < $1",
        ),
        (
            "webauthn_challenges",
            "DELETE FROM webauthn_challenges WHERE expires_at < $1",
        ),
        (
            "qr_sessions",
            "DELETE FROM qr_sessions WHERE expires_at < $1",
        ),
        (
            "login_grants",
            "DELETE FROM login_grants WHERE expires_at < $1",
        ),
    ] {
        let deleted = sqlx::query(sql)
            .bind(cutoff)
            .execute(db)
            .await?
            .rows_affected();
        if deleted > 0 {
            tracing::debug!(table, deleted, "cleaned up expired rows");
        }
    }

    prune_job_history(db).await?;
    Ok(())
}

/// Deletes old finished rows from the `jobs` table itself (see the module doc for why this is
/// needed). Pending, running and just-finished jobs are never touched.
async fn prune_job_history(db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM jobs WHERE status = 'succeeded' AND updated_at < $1")
        .bind(Utc::now() - SUCCEEDED_JOB_RETENTION)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM jobs WHERE status = 'dead' AND updated_at < $1")
        .bind(Utc::now() - DEAD_JOB_RETENTION)
        .execute(db)
        .await?;
    Ok(())
}

/// How long until this job should run again after a successful sweep.
pub fn reschedule_after() -> Duration {
    INTERVAL
}
