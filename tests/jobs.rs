//! Tests for the Postgres-backed job queue (`infra::jobs`). See `queue.md` for the design.

mod common;

use std::time::Duration;

use api_starter_axum::infra::jobs::{self, JobsRepository, cleanup};
use chrono::Utc;
use uuid::Uuid;

const KIND: &str = "test_kind";

#[tokio::test]
async fn a_pending_job_is_claimed_exactly_once_under_concurrency() {
    let pool = common::fresh_pool().await;
    let repo = JobsRepository::new(pool.clone());
    repo.enqueue(KIND, "{}", Utc::now()).await.unwrap();

    // Ten concurrent claims racing for the single job: `FOR UPDATE SKIP LOCKED` must hand it
    // to exactly one of them, and none may block waiting on another.
    let claims = futures_all(10, || {
        let repo = repo.clone();
        async move { repo.claim_batch(1).await.unwrap() }
    })
    .await;

    let claimed: Vec<_> = claims.into_iter().flatten().collect();
    assert_eq!(
        claimed.len(),
        1,
        "exactly one caller should have claimed the job"
    );
}

#[tokio::test]
async fn claim_only_returns_due_jobs_and_never_the_same_job_twice() {
    let pool = common::fresh_pool().await;
    let repo = JobsRepository::new(pool.clone());
    repo.enqueue(KIND, "{}", Utc::now() + chrono::Duration::hours(1))
        .await
        .unwrap(); // not due yet
    repo.enqueue(KIND, "{}", Utc::now()).await.unwrap(); // due

    let first = repo.claim_batch(10).await.unwrap();
    assert_eq!(first.len(), 1, "only the due job should be claimable");

    let second = repo.claim_batch(10).await.unwrap();
    assert!(second.is_empty(), "a claimed job cannot be claimed again");
}

#[tokio::test]
async fn failed_jobs_back_off_then_go_dead_after_max_attempts() {
    let pool = common::fresh_pool().await;
    let repo = JobsRepository::new(pool.clone());
    repo.enqueue(KIND, "{}", Utc::now()).await.unwrap();
    let job = repo
        .claim_batch(1)
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!((job.attempts, job.max_attempts), (0, 5));

    // First failure: rescheduled, not dead, `run_at` pushed into the future.
    let dead = repo
        .reschedule_or_kill(&job, "boom", chrono::Duration::seconds(30))
        .await
        .unwrap();
    assert!(!dead);
    assert!(
        repo.claim_batch(1).await.unwrap().is_empty(),
        "the retry is not due yet"
    );

    let (attempts, status, last_error): (i32, String, Option<String>) =
        sqlx::query_as("SELECT attempts, status, last_error FROM jobs WHERE id = $1")
            .bind(job.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (attempts, status.as_str(), last_error.as_deref()),
        (1, "pending", Some("boom"))
    );

    // Drive it to its last attempt: max_attempts is 5, so the 5th failure must go dead.
    let mut current = job.clone();
    current.attempts = 4;
    let dead = repo
        .reschedule_or_kill(&current, "boom again", chrono::Duration::seconds(0))
        .await
        .unwrap();
    assert!(dead, "the 5th attempt should exhaust max_attempts");

    let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(job.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "dead");
    // Dead jobs are kept (inspectable), not deleted, and never claimed again.
    assert!(repo.claim_batch(10).await.unwrap().is_empty());
}

#[tokio::test]
async fn ensure_scheduled_does_not_duplicate_a_pending_job() {
    let pool = common::fresh_pool().await;
    let repo = JobsRepository::new(pool.clone());

    repo.ensure_scheduled(cleanup::KIND).await.unwrap();
    repo.ensure_scheduled(cleanup::KIND).await.unwrap();
    repo.ensure_scheduled(cleanup::KIND).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE kind = $1")
        .bind(cleanup::KIND)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "a second call must not enqueue a duplicate");
}

#[tokio::test]
async fn cleanup_deletes_rows_expired_over_an_hour_ago_but_keeps_recent_ones() {
    let pool = common::fresh_pool().await;
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, created_at) VALUES ($1, $2, 'x', now())",
    )
    .bind(user_id)
    .bind(format!("{user_id}@example.com"))
    .execute(&pool)
    .await
    .unwrap();

    let insert_token = |expires_at: chrono::DateTime<Utc>| {
        let pool = pool.clone();
        async move {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO auth_tokens (id, user_id, purpose, token_hash, expires_at, created_at)
                 VALUES ($1, $2, 'password_reset', $3, $4, now())",
            )
            .bind(id)
            .bind(user_id)
            .bind(id.to_string())
            .bind(expires_at)
            .execute(&pool)
            .await
            .unwrap();
            id
        }
    };

    let long_expired = insert_token(Utc::now() - chrono::Duration::hours(2)).await;
    let just_expired = insert_token(Utc::now() - chrono::Duration::minutes(1)).await;
    let still_valid = insert_token(Utc::now() + chrono::Duration::hours(1)).await;

    cleanup::run(&pool).await.unwrap();

    let remaining: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM auth_tokens ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        !remaining.contains(&long_expired),
        "over an hour past expiry: swept"
    );
    assert!(
        remaining.contains(&just_expired),
        "within the 1-hour grace period: kept"
    );
    assert!(remaining.contains(&still_valid), "not expired: kept");
}

#[tokio::test]
async fn cleanup_prunes_its_own_old_history_but_keeps_recent_and_active_rows() {
    let pool = common::fresh_pool().await;
    let repo = JobsRepository::new(pool.clone());

    let insert_job = |status: &'static str, updated_at: chrono::DateTime<Utc>| {
        let pool = pool.clone();
        async move {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO jobs (id, kind, payload, status, run_at, created_at, updated_at)
                 VALUES ($1, $2, '{}', $3, now(), now(), $4)",
            )
            .bind(id)
            .bind(KIND)
            .bind(status)
            .bind(updated_at)
            .execute(&pool)
            .await
            .unwrap();
            id
        }
    };

    let old_succeeded = insert_job("succeeded", Utc::now() - chrono::Duration::days(2)).await;
    let recent_succeeded = insert_job("succeeded", Utc::now()).await;
    let old_dead = insert_job("dead", Utc::now() - chrono::Duration::days(60)).await;
    let recent_dead = insert_job("dead", Utc::now() - chrono::Duration::days(1)).await;
    repo.enqueue(KIND, "{}", Utc::now()).await.unwrap(); // an active job, unrelated to pruning

    cleanup::run(&pool).await.unwrap();

    let remaining: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM jobs WHERE kind = $1")
        .bind(KIND)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        !remaining.contains(&old_succeeded),
        "succeeded, over a day old: pruned"
    );
    assert!(
        remaining.contains(&recent_succeeded),
        "succeeded, recent: kept"
    );
    assert!(
        !remaining.contains(&old_dead),
        "dead, over a month old: pruned"
    );
    assert!(
        remaining.contains(&recent_dead),
        "dead, recent: kept for investigation"
    );
}

#[tokio::test]
async fn the_worker_claims_and_runs_a_job_end_to_end() {
    let pool = common::fresh_pool().await;
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, created_at) VALUES ($1, $2, 'x', now())",
    )
    .bind(user_id)
    .bind(format!("{user_id}@example.com"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO auth_tokens (id, user_id, purpose, token_hash, expires_at, created_at)
         VALUES ($1, $2, 'password_reset', 'expired-token', $3, now())",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(Utc::now() - chrono::Duration::hours(2))
    .execute(&pool)
    .await
    .unwrap();

    let worker = jobs::spawn(pool.clone());

    // The worker polls once a second; give it a few ticks to pick up the startup-scheduled
    // cleanup job and sweep the row, rather than asserting on a fixed sleep.
    let swept = wait_until(Duration::from_secs(5), || {
        let pool = pool.clone();
        async move {
            let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM auth_tokens")
                .fetch_one(&pool)
                .await
                .unwrap();
            remaining == 0
        }
    })
    .await;
    assert!(swept, "the worker should have swept the expired token");

    // The cleanup job reschedules itself, leaving its finished run behind as history: expect
    // exactly one *active* (still pending/running) schedule, not zero and not a duplicate.
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jobs WHERE kind = $1 AND status IN ('pending', 'running')",
    )
    .bind(cleanup::KIND)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active, 1,
        "cleanup should have re-enqueued itself exactly once"
    );

    worker.shutdown(Duration::from_secs(2)).await;
}

/// Runs `n` copies of `make_future()` concurrently and returns all of their results.
async fn futures_all<T, F, Fut>(n: usize, make_future: F) -> Vec<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let handles: Vec<_> = (0..n).map(|_| tokio::spawn(make_future())).collect();
    let mut out = Vec::with_capacity(n);
    for handle in handles {
        out.push(handle.await.unwrap());
    }
    out
}

/// Polls `condition` every 100ms until it is true or `timeout` elapses.
async fn wait_until<F, Fut>(timeout: Duration, mut condition: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if condition().await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
