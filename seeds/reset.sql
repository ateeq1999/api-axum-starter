-- Deletes ALL application data, keeping the schema. Used by:  cargo run -- seed --fresh
-- (which runs this script and then seeds/seed.sql, leaving exactly the seed data).
--
-- DEVELOPMENT ONLY. This removes every user and every one-time link token.

BEGIN;

DELETE FROM auth_tokens;
DELETE FROM users;

COMMIT;
