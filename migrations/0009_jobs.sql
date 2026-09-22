-- Durable background jobs. Claimed with `FOR UPDATE SKIP LOCKED` (see infra/jobs/repository.rs),
-- the standard Postgres queue pattern: no extension needed, safe under concurrent workers.
CREATE TABLE jobs (
    id           UUID PRIMARY KEY NOT NULL,
    kind         TEXT NOT NULL,                  -- e.g. 'cleanup_expired_rows'
    payload      TEXT NOT NULL DEFAULT '{}',      -- JSON, shape depends on `kind`
    status       TEXT NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'dead')),
    attempts     INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    run_at       TIMESTAMPTZ NOT NULL,            -- do not claim before this time (schedules + backoff)
    locked_at    TIMESTAMPTZ,
    last_error   TEXT,
    created_at   TIMESTAMPTZ NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL
);

-- Partial index: only pending/claimable rows matter for the claim query's WHERE + ORDER BY.
CREATE INDEX idx_jobs_claimable ON jobs (run_at) WHERE status = 'pending';
