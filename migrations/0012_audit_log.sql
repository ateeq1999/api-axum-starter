-- Durable, admin-readable record of security-sensitive administrative actions (role changes,
-- deactivation, deletion): tracing logs are not queryable and roll off, this does not.
CREATE TABLE audit_log (
    id             UUID PRIMARY KEY,
    actor_user_id  UUID NOT NULL,
    action         TEXT NOT NULL,
    target_user_id UUID,
    details        TEXT NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_audit_log_created_at ON audit_log (created_at DESC);
CREATE INDEX idx_audit_log_target_user_id ON audit_log (target_user_id);
