-- A soft-deleted account keeps its real email (for audit and restore) instead of having it
-- rewritten to a placeholder, so uniqueness must only apply to accounts that still exist. That
-- is what frees the address for a new registration after the old account is deleted.
--
-- Existing soft-deleted rows already hold placeholder emails from the previous behaviour; they
-- stay as they are.
ALTER TABLE users DROP CONSTRAINT users_email_key;

CREATE UNIQUE INDEX users_email_unique_among_active ON users (email) WHERE deleted_at IS NULL;
