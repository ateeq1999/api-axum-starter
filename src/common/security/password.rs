use std::sync::{Arc, OnceLock};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use tokio::sync::Semaphore;

use crate::common::error::{AppError, AppResult};

/// One argon2 hash allocates about 19 MiB, so peak memory is roughly
/// `max concurrent hashes x 19 MiB` no matter how much traffic arrives.
pub const DEFAULT_MAX_CONCURRENT_HASHES: usize = 8;

static PERMITS: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Sets the cap on simultaneous hash/verify operations. Call once at startup; later calls are
/// ignored (the first value wins), as are calls after the first hash has already run.
pub fn set_max_concurrent_hashes(max: usize) {
    let _ = PERMITS.set(Arc::new(Semaphore::new(max.max(1))));
}

fn permits() -> Arc<Semaphore> {
    PERMITS
        .get_or_init(|| Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT_HASHES)))
        .clone()
}

pub fn hash(password: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("password hashing failed: {e}")))
}

pub fn verify(password: &str, stored_hash: &str) -> bool {
    PasswordHash::new(stored_hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Runs CPU-heavy `work` on the blocking pool while holding one of `permits`.
///
/// The permit moves into the blocking task, so it is released only when the work really ends.
/// If the caller is cancelled meanwhile (request timeout, client disconnect) the work still
/// finishes but keeps counting against the cap, so the cap is never exceeded.
async fn run_limited<T: Send + 'static>(
    permits: Arc<Semaphore>,
    work: impl FnOnce() -> T + Send + 'static,
) -> AppResult<T> {
    let permit = permits
        .acquire_owned()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(|e| AppError::Internal(e.into()))
}

/// Argon2 is CPU-heavy; run it off the async executor threads, a bounded number at a time.
pub async fn hash_blocking(password: String) -> AppResult<String> {
    run_limited(permits(), move || hash(&password)).await?
}

pub async fn verify_blocking(password: String, stored_hash: String) -> AppResult<bool> {
    run_limited(permits(), move || verify(&password, &stored_hash)).await
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use super::*;

    #[test]
    fn hash_then_verify() {
        let h = hash("correct horse").unwrap();
        assert!(verify("correct horse", &h));
        assert!(!verify("wrong", &h));
    }

    #[tokio::test]
    async fn blocking_variants_agree() {
        let h = hash_blocking("s3cret-pass".into()).await.unwrap();
        assert!(
            verify_blocking("s3cret-pass".into(), h.clone())
                .await
                .unwrap()
        );
        assert!(!verify_blocking("nope".into(), h).await.unwrap());
    }

    #[tokio::test]
    async fn never_runs_more_than_the_cap_at_once() {
        let permits = Arc::new(Semaphore::new(2));
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let (permits, running, peak) = (permits.clone(), running.clone(), peak.clone());
                tokio::spawn(async move {
                    run_limited(permits, move || {
                        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(30));
                        running.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
                    .unwrap();
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_cancelled_caller_keeps_its_permit_until_the_work_ends() {
        let permits = Arc::new(Semaphore::new(1));
        let caller = tokio::spawn(run_limited(permits.clone(), || {
            std::thread::sleep(Duration::from_millis(150));
        }));
        tokio::time::sleep(Duration::from_millis(30)).await;
        caller.abort(); // like a request timeout dropping the handler future

        assert_eq!(permits.available_permits(), 0, "work is still running");
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            permits.available_permits(),
            1,
            "released once the work finished"
        );
    }
}
