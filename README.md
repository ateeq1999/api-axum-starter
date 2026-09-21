# api-starter-axum

A REST API starter built on [axum](https://github.com/tokio-rs/axum), SQLite (sqlx) and JWT auth. It ships with user management, password reset, email verification and email change, and sends mail through an SMTP server.

## Quick start

```bash
cp .env.example .env        # then set JWT_SECRET (openssl rand -hex 32)
cargo run
```

The server listens on `127.0.0.1:3000` by default. Migrations run automatically at startup and the SQLite file is created if missing.

```bash
curl -X POST localhost:3000/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"email":"me@example.com","password":"correct-horse-battery"}'

curl -X POST localhost:3000/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"me@example.com","password":"correct-horse-battery"}'
# => {"access_token":"...","token_type":"Bearer","expires_in":3600}

curl localhost:3000/api/v1/users/me -H "authorization: Bearer <access_token>"
```

With `MAIL_ENABLED=false` (the default) no email is sent; the message is logged instead. Set `RUST_LOG=debug` to see the body, including the link.

### First administrator

No admin exists after the first migration. Set `ADMIN_EMAIL` and `ADMIN_PASSWORD` and one is created at startup, only if no active admin exists yet.

### Seed data (development)

```bash
cargo run -- seed
```

Runs the migrations, executes [seeds/seed.sql](seeds/seed.sql) and exits (it does not start the server). Safe to run repeatedly: rows are only inserted if missing. It creates five accounts, all with the password `Password123!`:

| Email | Role | State |
|---|---|---|
| `admin@example.com` | admin | verified |
| `alice@example.com`, `bob@example.com` | user | verified |
| `carol@example.com` | user | email not verified |
| `dave@example.com` | user | deactivated (login is rejected) |

To start over from a clean slate, wipe everything and re-seed in one step:

```bash
cargo run -- seed --fresh          # asks you to type `yes`
cargo run -- seed --fresh --yes    # no prompt (required when there is no terminal, e.g. CI)
```

This runs [seeds/reset.sql](seeds/reset.sql) (deletes every user and every emailed-link token, keeps the schema) and then `seeds/seed.sql`, so afterwards the database holds exactly the seed accounts. It is destructive and for development only.

Development only: never run it against a production database. To change the data, edit `seeds/seed.sql` (UUIDs are stored as 16-byte blobs, so ids are `X'...'` literals). The script is compiled into the binary, so re-run `cargo run -- seed` after editing it.

## Configuration

All configuration is environment variables (a `.env` file is loaded if present; real environment variables win). The app fails fast at startup on missing or invalid values.

| Variable | Default | Purpose |
|---|---|---|
| `DATABASE_URL` | required | e.g. `sqlite://app.db` |
| `BIND_ADDR` | `127.0.0.1:3000` | Listen address |
| `JWT_SECRET` | required | At least 32 characters |
| `JWT_TTL_SECS` | `3600` | Access token lifetime |
| `MAIL_ENABLED` | `false` | `false` logs emails instead of sending |
| `SMTP_HOST` | required if mail enabled | Mail server host (Docker service name on a shared network) |
| `SMTP_PORT` | `587` | |
| `SMTP_TLS` | `starttls` | `none`, `starttls` or `tls` |
| `SMTP_USERNAME`, `SMTP_PASSWORD` | unset | Omit if the server trusts the network |
| `MAIL_FROM` | required if mail enabled | e.g. `Acme <no-reply@example.com>` |
| `FRONTEND_URL` | `http://localhost:5173` | Base URL for links in emails |
| `PASSWORD_RESET_TTL_MINUTES` | `30` | |
| `EMAIL_VERIFICATION_TTL_HOURS` | `24` | Also the lifetime of invitation links |
| `EMAIL_CHANGE_TTL_MINUTES` | `60` | |
| `REQUIRE_VERIFIED_EMAIL` | `false` | Block login until the email is verified |
| `RATE_LIMIT_PER_MINUTE` | `20` | Per client IP, for login, register and the reset/verify endpoints |
| `MAX_CONCURRENT_HASHES` | `8` | Max simultaneous argon2 operations (login, register, reset, change password). Each allocates ~19 MiB, so peak memory is about this number x 19 MiB |
| `ADMIN_EMAIL`, `ADMIN_PASSWORD` | unset | Bootstrap administrator (set both or neither) |
| `LOG_FORMAT` | text | `json` for structured logs |
| `RUST_LOG` | see `infra/telemetry.rs` | Log filter |

## API

Base path: `/api/v1`. Errors are always `{"error": {"code", "message", "details"?}}`. Validation failures return `422`.

### Auth (`/auth`)

| Method | Path | Access | Purpose |
|---|---|---|---|
| POST | `/register` | public, rate limited | Create an account; sends a verification email |
| POST | `/login` | public, rate limited | Returns a bearer token |
| GET | `/me` | authenticated | Current user |
| POST | `/password/forgot` | public, rate limited | Email a reset link. Always `202`, whether or not the address exists |
| POST | `/password/reset` | public, rate limited | `{token, new_password}` |
| POST | `/password/change` | authenticated | `{current_password, new_password}` |
| POST | `/email/verify` | public, rate limited | `{token}` |
| POST | `/email/verification/resend` | authenticated, rate limited | New verification link (no-op if already verified) |
| POST | `/email/change` | authenticated | `{new_email, current_password}`; link goes to the new address, notice to the old |
| POST | `/email/change/confirm` | public, rate limited | `{token}` |
| POST | `/invitations` | admin | `{email, display_name?, role?}`; emails a "set your password" link |

### Users (`/users`)

| Method | Path | Access | Purpose |
|---|---|---|---|
| GET | `/` | admin | List. Query: `page`, `per_page` (max 100), `q`, `sort` (`created_at`\|`email`), `order` (`asc`\|`desc`) |
| POST | `/` | admin | Create with a chosen password |
| GET | `/me` | authenticated | Own profile |
| PATCH | `/me` | authenticated | Update own `display_name` |
| GET | `/{id}` | admin or the user themself | |
| PATCH | `/{id}` | admin | `display_name`, `role`, `is_active` |
| DELETE | `/{id}` | admin | Soft delete, `204` |

Rules enforced: an admin cannot demote, deactivate or delete themself, and the last active admin cannot be removed.

Health checks: `GET /health/live` and `GET /health/ready` (checks the database).

## Project layout

Each feature is a folder under `modules/` and owns its controller, service, repository, DTOs and errors. The folder names the feature and the file names the role, so `modules/auth/services/password_reset.rs` reads as "auth service: password reset". A role is one file while it has one thing in it, and becomes a folder once it has two or more.

```
src/
├── main.rs, lib.rs, app.rs, state.rs   boot, router assembly, AppState (holds the services)
├── config/                             environment-driven Config
├── infra/                              database pool + migrations, tracing setup
├── common/                             feature-agnostic building blocks
│   ├── error.rs                        AppError -> HTTP response
│   ├── dto/                            Pagination, MessageResponse
│   ├── extractors/                     ValidatedJson, ValidatedQuery
│   ├── security/                       jwt, password hashing, Role, AuthUser, AdminUser
│   └── middleware/                     request id, tracing, rate limiter, global layer stack
└── modules/
    ├── health/
    ├── mail/                           no HTTP; MailService, SMTP transport, messages/, templates/
    ├── users/                          controller, service, policy, repository, entity, dto, error
    └── auth/                           controllers/, services/, repositories/, dto/, helpers/, entity, error
migrations/                             0001 users, 0002 users management, 0003 auth tokens
seeds/seed.sql, seeds/reset.sql         development seed data / wipe (`cargo run -- seed [--fresh]`)
tests/                                  integration tests (see Testing)
```

Layers, one direction only:

```
controller  ->  service  ->  repository  ->  DB
  axum types     rules          SQL
```

- Controllers only use axum types and call one service method. No SQL or hashing.
- Services hold the business rules and know nothing about axum.
- Repositories hold all SQL.
- DTOs are the request and response shapes; entities are never serialized directly.

Modules depend on each other in one direction: `auth` uses `users` and `mail`; `users` and `mail` use only `common`. That is why the JWT, password and `AuthUser`/`AdminUser` code lives in `common/security/` rather than in `auth`.

Axum extractors play the role of guards and pipes: `AuthUser` and `AdminUser` are guards, `ValidatedJson<T>` is the validation pipe, and tower layers in `common/middleware/` are the middleware. Failures are named errors (`AuthError::InvalidCredentials`, `UsersError::EmailTaken`, ...) that each convert into `AppError`, the single place that maps errors to HTTP status codes.

### Adding a feature

1. Create `modules/<name>/` with `mod.rs`, `controller.rs`, `service.rs`, `repository.rs`, `entity.rs`, `dto/`, `error.rs` (`impl From<YourError> for AppError`).
2. Add the migration under `migrations/` (`0004_<name>.sql`).
3. Build the service in `AppState::with_mail` (`state.rs`) and add it as an `Arc<YourService>` field; `#[derive(FromRef)]` lets handlers take `State<Arc<YourService>>`. Keep every `AppState` field a cheap handle (`Arc`, pool): axum clones the state on every request, so a `String` or `Vec` field would be copied each time.
4. Nest its router in `modules/mod.rs`.

New emails: add a struct in `modules/mail/messages/`, a `<name>.html` and `<name>.txt` in `modules/mail/templates/`, and a `send_<name>` method on `MailService`.

## How the account flows work

Reset, verification and email-change links share one mechanism, the `auth_tokens` table:

- The token is 32 random bytes (base64url) and exists only inside the emailed link. The database stores its SHA-256.
- A token works once (claimed with a single atomic `UPDATE ... RETURNING`) and only before it expires.
- Issuing a new token of the same kind for a user invalidates the older ones.
- `forgot` answers identically for known and unknown addresses, sends the mail in the background, and sends at most 3 reset emails per account per hour.
- Following a reset or invitation link also marks the email verified, since it proves control of the inbox.

Links point at your frontend: `{FRONTEND_URL}/reset-password?token=...`, `/verify-email?token=...`, `/confirm-email-change?token=...`. The frontend page then POSTs the token to the matching endpoint.

## Mail

`MailService` renders each email as text plus HTML and sends it in a background task with up to 3 attempts (1 s and 4 s backoff) for transient SMTP errors. A mail failure is logged (without the link) and never fails a request. Pending mail is given 5 seconds to drain on shutdown.

To use your SMTP server, set `MAIL_ENABLED=true`, `SMTP_HOST`, `SMTP_PORT`, `SMTP_TLS`, `MAIL_FROM` and, if required, `SMTP_USERNAME`/`SMTP_PASSWORD`. TLS uses the system trust store; for a private CA set `SSL_CERT_FILE`, or use `SMTP_TLS=none` on a private Docker network. For mail to reach inboxes, the sending domain needs SPF, DKIM and DMARC records.

To catch mail locally instead of sending it:

```bash
docker compose -f docker-compose.dev.yml up -d   # Mailpit: SMTP :1025, web UI http://localhost:8025
# .env: MAIL_ENABLED=true  SMTP_HOST=localhost  SMTP_PORT=1025  SMTP_TLS=none
```

## Testing

```bash
cargo test
cargo clippy --all-targets
```

- Unit tests sit next to the code (token hashing, policy rules, pagination, rate limiter, templates, ...).
- `tests/auth.rs` and `tests/users.rs` drive the full router in-process against an in-memory SQLite database, with an in-memory mail transport so tests can read the emailed links.
- `tests/mail_smtp.rs` runs the real SMTP transport against a small fake SMTP server.

## Known limits

- Access tokens are stateless. Demoting or deactivating a user, or changing a password, does not revoke tokens already issued; they expire after `JWT_TTL_SECS`.
- The rate limiter is in-process and keyed on the TCP peer address. Behind a reverse proxy every request shares the proxy's address, so limit at the proxy; with several replicas the limits are per replica.
- Mail delivery is best-effort. A crash between creating a link and sending the email loses that email; the user can request another.
- The "last admin" check is not serialized against concurrent requests.
- Soft delete rewrites the user's email to `deleted+<id>@deleted.invalid` so the address can be registered again.
- Password hashing is capped at `MAX_CONCURRENT_HASHES` at a time (default 8), so a burst of logins queues instead of allocating 19 MiB each. Under a burst, requests wait for a free slot; the 10 s request timeout still applies.
