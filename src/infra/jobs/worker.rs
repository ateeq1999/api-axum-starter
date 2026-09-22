use std::{sync::Arc, time::Duration as StdDuration};

use chrono::Duration;
use sqlx::PgPool;
use tokio::sync::{Notify, Semaphore};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{cleanup, repository::Job, repository::JobsRepository};

const POLL_INTERVAL: StdDuration = StdDuration::from_secs(1);
const MAX_CONCURRENT_JOBS: usize = 4;
const CLAIM_BATCH: i64 = 8;
/// Backoff after a failed attempt: 1s, 4s, 16s, then every 16s until `max_attempts` is hit.
const RETRY_BACKOFF: [i64; 3] = [1, 4, 16];

/// The background worker: polls for due jobs, runs them with bounded concurrency, and drains
/// in-flight work on shutdown. One instance runs inside the same process as the web server,
/// exactly like mail delivery does — no separate worker process to deploy.
#[derive(Clone)]
pub struct Worker {
    repo: JobsRepository,
    shutdown: CancellationToken,
    tasks: TaskTracker,
}

pub fn spawn(db: PgPool) -> Worker {
    let worker = Worker {
        repo: JobsRepository::new(db.clone()),
        shutdown: CancellationToken::new(),
        tasks: TaskTracker::new(),
    };
    let handle = worker.clone();
    tokio::spawn(async move { handle.run(db).await });
    worker
}

impl Worker {
    /// Waits (bounded) for in-flight jobs so shutdown does not cut one off mid-run.
    pub async fn shutdown(&self, timeout: StdDuration) {
        self.shutdown.cancel();
        self.tasks.close();
        if tokio::time::timeout(timeout, self.tasks.wait())
            .await
            .is_err()
        {
            tracing::warn!("timed out waiting for jobs to finish");
        }
    }

    async fn run(&self, db: PgPool) {
        if let Err(error) = self.repo.ensure_scheduled(cleanup::KIND).await {
            tracing::error!(%error, "could not schedule the initial cleanup job");
        }

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_JOBS));
        let notify = Arc::new(Notify::new());
        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                _ = tokio::time::sleep(POLL_INTERVAL) => {},
                _ = notify.notified() => {},
            }
            if self.shutdown.is_cancelled() {
                break;
            }

            let jobs = match self.repo.claim_batch(CLAIM_BATCH).await {
                Ok(jobs) => jobs,
                Err(error) => {
                    tracing::error!(%error, "could not claim jobs");
                    continue;
                }
            };
            for job in jobs {
                let permit = semaphore.clone().acquire_owned().await.unwrap();
                let repo = self.repo.clone();
                let db = db.clone();
                let notify = notify.clone();
                self.tasks.spawn(async move {
                    let _permit = permit;
                    process(&repo, &db, job).await;
                    // Wake the poll loop immediately in case more jobs are already due
                    // (e.g. a chain of self-rescheduling jobs), instead of waiting a full tick.
                    notify.notify_one();
                });
            }
        }
    }
}

async fn process(repo: &JobsRepository, db: &PgPool, job: Job) {
    let result = dispatch(db, &job).await;
    match result {
        Ok(()) => {
            if let Err(error) = repo.mark_succeeded(job.id).await {
                tracing::error!(%error, job_id = %job.id, "could not mark job succeeded");
            }
        }
        Err(error) => {
            let backoff = Duration::seconds(
                RETRY_BACKOFF[(job.attempts as usize).min(RETRY_BACKOFF.len() - 1)],
            );
            match repo.reschedule_or_kill(&job, &error, backoff).await {
                Ok(true) => tracing::error!(
                    job_id = %job.id, kind = %job.kind, %error,
                    "job failed permanently and was moved to the dead letter state"
                ),
                Ok(false) => tracing::warn!(
                    job_id = %job.id, kind = %job.kind, attempt = job.attempts + 1, %error,
                    "job failed, will retry"
                ),
                Err(error) => {
                    tracing::error!(%error, job_id = %job.id, "could not reschedule failed job")
                }
            }
        }
    }
}

/// Add a new job kind here (and give it a home module, following `cleanup`'s shape).
async fn dispatch(db: &PgPool, job: &Job) -> Result<(), String> {
    match job.kind.as_str() {
        cleanup::KIND => {
            cleanup::run(db).await.map_err(|e| e.to_string())?;
            let repo = JobsRepository::new(db.clone());
            repo.enqueue(
                cleanup::KIND,
                "{}",
                chrono::Utc::now() + cleanup::reschedule_after(),
            )
            .await
            .map_err(|e| e.to_string())
        }
        other => Err(format!("no handler registered for job kind `{other}`")),
    }
}
