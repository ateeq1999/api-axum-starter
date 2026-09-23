-- Durable record of an email between "rendered" and "delivered". A row is inserted before the
-- send is attempted and deleted once it succeeds; a row still `pending` after a crash is resent
-- at the next startup instead of being silently lost (see modules::mail::repository).
CREATE TABLE outbound_mail (
    id         UUID PRIMARY KEY,
    payload    TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'dead')),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_outbound_mail_pending ON outbound_mail (created_at) WHERE status = 'pending';
