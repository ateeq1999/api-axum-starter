//! A small durable job queue backed by Postgres, claimed with `FOR UPDATE SKIP LOCKED` —
//! the standard Postgres queue pattern (the same mechanism the `pgmq` extension itself uses
//! internally), needing no extension. See `queue.md` at the repo root for the design rationale.
//!
//! Used today for one recurring job, [`cleanup::CLEANUP_EXPIRED_ROWS`], which sweeps expired
//! rows on a timer by re-enqueuing itself — a "cron job" that needs no `pg_cron` extension.
//! Add a new job kind by matching on it in [`worker::dispatch`].

pub mod cleanup;
pub mod repository;
pub mod worker;

pub use repository::{Job, JobsRepository};
pub use worker::{Worker, spawn};
