-- Development seed data. Run it with:  cargo run -- seed
--
-- * Safe to run more than once: a row is only inserted if its id / email is missing, and
--   existing rows are never modified.
-- * Ids are 16-byte blobs (X'...' literals), which is how the app stores UUIDs.
-- * Every account below has the password  Password123!
-- * DEVELOPMENT ONLY. Never run this against a production database.

BEGIN;

INSERT OR IGNORE INTO users
    (id, email, password_hash, display_name, role, is_active, email_verified_at, created_at, updated_at)
VALUES
    -- administrator
    (X'00000000000040008000000000000001', 'admin@example.com',
     '$argon2id$v=19$m=19456,t=2,p=1$DpnToHrrH7OkFeMnsigOGw$MN2d8/UO/DC/U4jDUeJN7k7pwMHWn/sKzTOrrTBtQcU',
     'Admin', 'admin', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', NULL),
    -- regular, verified users
    (X'00000000000040008000000000000002', 'alice@example.com',
     '$argon2id$v=19$m=19456,t=2,p=1$DpnToHrrH7OkFeMnsigOGw$MN2d8/UO/DC/U4jDUeJN7k7pwMHWn/sKzTOrrTBtQcU',
     'Alice Johnson', 'user', 1, '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z', NULL),
    (X'00000000000040008000000000000003', 'bob@example.com',
     '$argon2id$v=19$m=19456,t=2,p=1$DpnToHrrH7OkFeMnsigOGw$MN2d8/UO/DC/U4jDUeJN7k7pwMHWn/sKzTOrrTBtQcU',
     'Bob Smith', 'user', 1, '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z', NULL),
    -- email not verified yet (cannot log in when REQUIRE_VERIFIED_EMAIL=true)
    (X'00000000000040008000000000000004', 'carol@example.com',
     '$argon2id$v=19$m=19456,t=2,p=1$DpnToHrrH7OkFeMnsigOGw$MN2d8/UO/DC/U4jDUeJN7k7pwMHWn/sKzTOrrTBtQcU',
     'Carol White', 'user', 1, NULL, '2026-01-04T00:00:00Z', NULL),
    -- deactivated account (login is rejected)
    (X'00000000000040008000000000000005', 'dave@example.com',
     '$argon2id$v=19$m=19456,t=2,p=1$DpnToHrrH7OkFeMnsigOGw$MN2d8/UO/DC/U4jDUeJN7k7pwMHWn/sKzTOrrTBtQcU',
     'Dave Brown', 'user', 0, '2026-01-05T00:00:00Z', '2026-01-05T00:00:00Z', NULL);

COMMIT;
