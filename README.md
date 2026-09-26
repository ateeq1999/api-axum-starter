# api-starter-axum

A REST API starter built on [axum](https://github.com/tokio-rs/axum), PostgreSQL (sqlx) and JWT auth. It ships with user management, password reset, email verification and email change (mail through an SMTP server), profile photos and general media uploads (local disk or S3), Google and GitHub sign-in, passkeys (WebAuthn), WhatsApp-style QR-code sign-in, TOTP two-factor authentication, API keys, an admin-readable audit log, a durable Postgres-backed background job queue, Prometheus metrics, and interactive API docs (Swagger UI).

Setting up OAuth, passkeys, QR login and the rest: see **[steps.md](steps.md)**.

- [Quick start](#quick-start)
- [Configuration](#configuration)
- [API](#api)
- [Project layout](#project-layout)
- [How the account flows work](#how-the-account-flows-work)
- [Mail](#mail)
- [Background jobs](#background-jobs)
- [Monitoring](#monitoring)
- [Deployment and CI](#deployment-and-ci)
- [Testing](#testing)
- [Known limits](#known-limits)

## Quick start

```bash
createdb api_starter_db      # or: psql -c 'CREATE DATABASE api_starter_db'
cargo run -- setup           # creates .env, fills in secrets, runs migrations
cargo run
```

`setup` copies `.env.example` to `.env` if missing, generates `JWT_SECRET`/`METRICS_TOKEN`, and runs migrations against the Postgres database named in `DATABASE_URL` (the app does not create the database itself — do that first). Add `--seed` (`cargo run -- setup --seed`) to also load the development [seed data](#seed-data-development) in the same step. The server then listens on `127.0.0.1:3000` by default.

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

Interactive API docs (Swagger UI) are served at `/docs`, and the raw spec at `/api-docs/openapi.json` — try requests directly from the browser once you have a token. See [API](#api).

### Running with Docker

```bash
docker build -t api-starter-axum .
docker run --env-file .env -p 3000:3000 -p 9091:9091 api-starter-axum
```

Migrations are embedded into the binary at compile time, so the image needs nothing from `migrations/`. It still needs a reachable Postgres (`DATABASE_URL`) and a database created ahead of time, same as running locally.

### First administrator

No admin exists after the first migration. Set `ADMIN_EMAIL` and `ADMIN_PASSWORD` and one is created at startup, only if no active admin exists yet.

### Generating secrets

`cargo run -- setup` already does this once for you on a fresh checkout. Run it again on its own whenever you want to rotate a secret later:

```bash
cargo run -- gen --secrets          # fills in JWT_SECRET and METRICS_TOKEN if unset
cargo run -- gen --secrets --force  # also overwrites ones already set
```

Fills in random values for `JWT_SECRET`, `METRICS_TOKEN` and (if `ADMIN_EMAIL` is already set) `ADMIN_PASSWORD` in `.env`, leaving anything already there untouched unless `--force` is given. Also updates `monitoring/prometheus.yml`'s scrape `credentials:` to match a newly generated `METRICS_TOKEN`, so local Prometheus scraping does not start failing with `401` right after rotating it.

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

All configuration is environment variables (a `.env` file is loaded if present; real environment variables win). The app fails fast at startup on missing or invalid values. Grouped here the same way as `.env.example`, where every variable is a live line: copy the file to `.env` and fill in values. A **blank** value counts as unset (the default applies, or the optional feature stays off), so unused lines can stay empty. `DATABASE_URL` and `JWT_SECRET` are the only ones the app cannot start without (`cargo run -- setup` generates the secret for you).

### Core & server

| Variable | Default | Purpose |
|---|---|---|
| `DATABASE_URL` | required | e.g. `postgres://postgres:postgres@localhost:5432/api_starter_db` |
| `DATABASE_MAX_CONNECTIONS` | `10` | Postgres connection pool size |
| `BIND_ADDR` | `127.0.0.1:3000` | Listen address |
| `CORS_ALLOWED_ORIGINS` | `FRONTEND_URL` | Comma-separated browser origins allowed cross-origin. `*` allows any (dev only) |
| `MAX_REQUEST_BODY_BYTES` | `10485760` (10 MiB) | Largest request body accepted anywhere in the API |
| `JWT_SECRET` | required | At least 32 characters |
| `JWT_TTL_SECS` | `3600` | Access token lifetime |
| `LOG_FORMAT` | text | `json` for structured logs |
| `RUST_LOG` | see `infra/telemetry.rs` | Log filter |

### Mail (SMTP)

| Variable | Default | Purpose |
|---|---|---|
| `MAIL_ENABLED` | `false` | `false` logs emails instead of sending |
| `SMTP_HOST` | required if mail enabled | Mail server host (Docker service name on a shared network) |
| `SMTP_PORT` | `587` | |
| `SMTP_TLS` | `starttls` | `none`, `starttls` or `tls` |
| `SMTP_USERNAME`, `SMTP_PASSWORD` | unset | Omit if the server trusts the network |
| `MAIL_FROM` | required if mail enabled | e.g. `Acme <no-reply@example.com>` |

### Account & session security

| Variable | Default | Purpose |
|---|---|---|
| `FRONTEND_URL` | `http://localhost:5173` | Base URL for links in emails (also the default origin for CORS/passkeys) |
| `PASSWORD_RESET_TTL_MINUTES` | `30` | |
| `EMAIL_VERIFICATION_TTL_HOURS` | `24` | Also the lifetime of invitation links |
| `EMAIL_CHANGE_TTL_MINUTES` | `60` | |
| `REQUIRE_VERIFIED_EMAIL` | `false` | Block login until the email is verified |
| `RATE_LIMIT_PER_MINUTE` | `20` | Per client IP, for login, register and the reset/verify endpoints |
| `TRUST_PROXY_HEADERS` | `false` | Key the client IP off `X-Forwarded-For` instead of the TCP peer. Only set behind exactly one trusted reverse proxy |
| `CHECK_PASSWORD_BREACHES` | `false` | Also reject passwords found in the Have I Been Pwned breach database (k-anonymity; only a 5-char hash prefix is sent) |
| `MAX_FAILED_LOGIN_ATTEMPTS` | `5` | Wrong-password attempts per account before it is temporarily locked |
| `ACCOUNT_LOCKOUT_MINUTES` | `15` | How long an account stays locked once the threshold above is hit |
| `MAX_CONCURRENT_HASHES` | `8` | Max simultaneous argon2 operations (login, register, reset, change password). Each allocates ~19 MiB, so peak memory is about this number x 19 MiB |

### Admin bootstrap

| Variable | Default | Purpose |
|---|---|---|
| `ADMIN_EMAIL`, `ADMIN_PASSWORD` | unset | Bootstrap administrator (set both or neither) |

### File storage (uploads)

| Variable | Default | Purpose |
|---|---|---|
| `UPLOAD_DIR` | `./uploads` | Local-disk root for every upload (profile photos and media). Ignored when `S3_BUCKET` is set |
| `S3_BUCKET` | unset | Store uploads in this S3 bucket instead of local disk (keys: `avatars/...`, `media/...`). Credentials, region and endpoint come from the standard `AWS_*` variables (or an EC2 instance role); the bucket is checked at startup |
| `S3_FORCE_PATH_STYLE` | on if `AWS_ENDPOINT_URL` is set | `endpoint/bucket/key` addressing, needed by local emulators (floci, LocalStack, MinIO) |
| `MEDIA_MAX_UPLOAD_BYTES` | `8388608` (8 MiB) | Largest single media upload. Must not exceed `MAX_REQUEST_BODY_BYTES` |
| `MEDIA_ALLOWED_CONTENT_TYPES` | jpeg, png, gif, webp, pdf, mp4, webm, mp3, ogg | Comma-separated content types users may upload, matched against the file's real bytes. SVG and HTML are deliberately absent |

### OAuth sign-in

| Variable | Default | Purpose |
|---|---|---|
| `PUBLIC_API_URL` | `http://localhost:<BIND_ADDR port>` | Public base URL of this API (OAuth redirect URIs are built from it) |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | unset | Enables Google sign-in (set both) |
| `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` | unset | Enables GitHub sign-in (set both) |

### Passkeys (WebAuthn)

| Variable | Default | Purpose |
|---|---|---|
| `WEBAUTHN_RP_ID` | host of `FRONTEND_URL` | Passkey relying-party domain (changing it invalidates every passkey) |
| `WEBAUTHN_RP_NAME` | `api-starter-axum` | Name the OS shows when creating a passkey |
| `WEBAUTHN_ORIGIN` | origin of `FRONTEND_URL` | Exact origin of the page that uses passkeys |

### QR-code sign-in

| Variable | Default | Purpose |
|---|---|---|
| `QR_LOGIN_TTL_SECS` | `120` | Lifetime of a sign-in QR code (30 to 900) |
| `QR_POLL_RATE_LIMIT_PER_MINUTE` | `120` | Per-IP limit for polling a QR session |

### Metrics (Prometheus)

| Variable | Default | Purpose |
|---|---|---|
| `METRICS_BIND_ADDR` | `127.0.0.1:9091` | Prometheus `/metrics`, served on its own listener |
| `METRICS_TOKEN` | unset | If set, `/metrics` requires `Authorization: Bearer <token>` |

## API

Base path: `/api/v1`. Errors are always `{"error": {"code", "message", "details"?}}`. Validation failures return `422`. Every endpoint is documented interactively at **`/docs`** (Swagger UI; spec at `/api-docs/openapi.json` — see `infra/openapi.rs`). WebAuthn ceremony payloads (passkey registration/login) show there as opaque JSON objects rather than modeled field-by-field: they are browser-generated blobs (`credential.toJSON()`), not meant for manual construction.

**Authenticating.** Send `Authorization: Bearer <access token>` (from any sign-in method) or an API key as `X-API-Key: ak_...` / `Authorization: Bearer ak_...`. API keys are `read` (safe HTTP methods only) or `write`, and can never manage credentials: passwords, emails, API keys, passkeys, OAuth links and approving QR logins need an interactive sign-in (marked *session* below).

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

`POST /login` returns `{access_token, token_type, expires_in}` normally, or `{requires_totp: true, pending_token}` if the account has two-factor enabled — finish with `POST /2fa/verify` below.

### Two-factor authentication (`/auth/2fa`)

| Method | Path | Access | Purpose |
|---|---|---|---|
| POST | `/setup` | session | Generates a secret; returns it plus a QR code (`qr_code_data_uri`) and a provisioning URI. Not enforced yet |
| POST | `/enable` | session | `{code}` from the authenticator app. Turns 2FA on and returns 8 one-time recovery codes, shown once |
| POST | `/disable` | session | `{current_password}` — required so a stolen session token alone cannot turn it off |
| POST | `/verify` | public, rate limited | `{pending_token, code}` — `code` is a live authenticator code or an `XXXXX-XXXXX` recovery code. Returns a real access token |

A wrong code does not spend `pending_token`: it stays usable for another attempt until it expires (5 minutes) or a correct code is given.

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

Rules enforced: an admin cannot demote, deactivate or delete themself, and the last active admin cannot be removed — that last check runs atomically (a Postgres advisory lock guards it), so two concurrent admin changes can never both succeed and leave zero admins.

### Profile photo

| Method | Path | Access | Purpose |
|---|---|---|---|
| PUT | `/users/me/avatar` | authenticated | Body: the raw image (PNG, JPEG, WebP or GIF, up to 2 MiB). Stored as a 256x256 JPEG. Returns the user with `avatar_url` |
| DELETE | `/users/me/avatar` | authenticated | Remove the photo |
| GET | `/avatars/{file}` | public | The image (`avatar_url` points here). Cacheable for a year: the name changes on every upload |

### Media (`/media`)

General file uploads for any user: images, PDFs, short audio/video. Files are stored through the shared `ObjectStorage` service (`infra/storage.rs`), the same one profile photos use.

| Method | Path | Access | Purpose |
|---|---|---|---|
| POST | `/` | authenticated | Body: the raw file bytes; optional `?filename=`. Returns `201` with `{id, content_type, size_bytes, original_filename, url, created_at}` |
| GET | `/` | authenticated | Your files, newest first. Query: `page`, `per_page` |
| GET | `/{id}` | owner or admin | Metadata |
| GET | `/{id}/content` | owner or admin | The file itself |
| DELETE | `/{id}` | owner or admin | Delete the file and its stored object, `204` |

Uploads are untrusted content served from the API's own origin, so:

- The type is **detected from the bytes** (magic numbers) and checked against `MEDIA_ALLOWED_CONTENT_TYPES`; the client's `Content-Type` is ignored. An unrecognized or disallowed type is `415`, an oversized file `413`, an empty body `400`.
- Only images, video and audio are shown inline; everything else (PDFs, ...) is sent as a download. Every response carries `X-Content-Type-Options: nosniff` and a sandboxing `Content-Security-Policy`.
- Files are private: another user gets `404` (existence is not leaked). Serving needs credentials, so a browser `<img>` cannot load one directly. Fetch it with the `Authorization` header and show a blob URL, or add a public variant if you need one.
- Deleting a user account deletes their media too.

Storage is local disk by default, or S3 when `S3_BUCKET` is set (uploaded with the AWS SDK, private objects, served through the API so the bucket needs no public access). To try S3 locally against an emulator such as floci:

```bash
export AWS_ENDPOINT_URL=http://localhost:4566 AWS_DEFAULT_REGION=us-east-1 AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test
S3_BUCKET=my-bucket cargo run                               # the bucket must already exist
TEST_S3_BUCKET=my-bucket cargo test --test media --test avatars s3   # S3 integration tests (skipped when unset)
```

### API keys (`/api-keys`)

| Method | Path | Access | Purpose |
|---|---|---|---|
| POST | `/` | session | `{name, scope?: "read"\|"write", expires_in_days?}`. The secret `key` is returned once |
| GET | `/` | session | List your keys (prefix, scope, expiry, last use; never the secret) |
| DELETE | `/{id}` | session | Revoke, `204` |

### OAuth sign-in (`/auth/oauth`, Google and GitHub)

| Method | Path | Access | Purpose |
|---|---|---|---|
| GET | `/providers` | public | Enabled providers |
| GET | `/{provider}/login?redirect=/path` | public, rate limited | Browser navigation: redirects to the provider |
| GET | `/{provider}/callback` | public, rate limited | The provider's redirect target; forwards to `FRONTEND_URL/oauth/callback?code=...` (or `?error=...`) |
| POST | `/exchange` | public, rate limited | `{code}`: trades the one-time code for an access token |
| POST | `/{provider}/link` | session | Returns `{authorize_url}` to link a provider to your account |
| GET | `/identities` | session | Linked provider accounts |
| DELETE | `/{provider}` | session | Unlink (refused if it is your last sign-in method) |

### Passkeys (`/auth/passkeys`)

| Method | Path | Access | Purpose |
|---|---|---|---|
| POST | `/register/begin` | session | Returns `{challenge_id, options}` for `navigator.credentials.create` |
| POST | `/register/finish` | session | `{challenge_id, name?, credential}` |
| POST | `/login/begin` | public, rate limited | Usernameless sign-in: `{challenge_id, options}` for `navigator.credentials.get` |
| POST | `/login/finish` | public, rate limited | `{challenge_id, credential}`: returns an access token |
| GET | `/` | session | Your passkeys |
| DELETE | `/{id}` | session | Remove (refused if it is your last sign-in method) |

### QR-code sign-in (`/auth/qr`)

The device that wants to sign in shows a QR code; an already signed-in device (the phone app) scans and approves it.

| Method | Path | Access | Purpose |
|---|---|---|---|
| POST | `/sessions` | public, rate limited | New device: returns `id`, `poll_secret`, `verification_code`, `qr_payload`, `qr_svg` |
| GET | `/sessions/{id}` | public (header `X-QR-Secret`) | New device: poll. `pending`, `scanned`, `approved` (with the access token, once), `rejected`, `expired`, `consumed` |
| POST | `/sessions/{id}/scan` | session | Phone: claim the QR code; returns where the request came from |
| POST | `/sessions/{id}/approve` | session | Phone: `{code}`, the 4-digit code shown on the new device |
| POST | `/sessions/{id}/reject` | session | Phone: refuse |

### Audit log (`/audit-log`)

| Method | Path | Access | Purpose |
|---|---|---|---|
| GET | `/` | admin | Paginated, newest first. Records `user.created_by_admin`, `user.role_changed`, `user.active_status_changed`, `user.deleted`, each with `actor_user_id`, `target_user_id` and a `details` JSON blob |

Recording never fails the underlying action — a logging hiccup must not block an admin from, say, deactivating a compromised account. It deliberately covers only admin actions on users for now; extending it to another module is one `AuditLogService::record` call (`modules::audit_log`).

Health checks: `GET /health/live` and `GET /health/ready` (checks the database).

## Project layout

Each feature is a folder under `modules/` and owns its controller, service, repository, DTOs and errors. The folder names the feature and the file names the role, so `modules/auth/services/password_reset.rs` reads as "auth service: password reset". A role is one file while it has one thing in it, and becomes a folder once it has two or more.

```
src/
├── main.rs                             ~5 lines: load .env, call cli::run()
├── lib.rs, app.rs, state.rs            module tree, router assembly, AppState (holds the services)
├── cli/                                one file per subcommand: serve, seed, setup, generate_secrets (`gen --secrets`)
├── config/                             environment-driven Config
├── infra/                              database pool + migrations, tracing setup, jobs/ (background job queue),
│                                        secrets.rs (.env generation), metrics.rs (Prometheus), openapi.rs (Swagger),
│                                        storage.rs (ObjectStorage: local disk or S3, shared by avatars and media)
├── common/                             feature-agnostic building blocks
│   ├── error.rs                        AppError -> HTTP response
│   ├── dto/                            Pagination, MessageResponse
│   ├── extractors/                     ValidatedJson, ValidatedQuery, ClientMeta
│   ├── security/                       jwt, password hashing/policy, totp, Role, AuthUser, SessionUser, AdminUser,
│   │                                    SessionAuth/ApiKeyAuth hooks
│   ├── net.rs                          trusted-proxy-aware client IP resolution
│   └── middleware/                     request id, tracing, rate limiter, global layer stack (CORS, body limit, ...)
└── modules/
    ├── health/
    ├── mail/                           no HTTP; MailService, SMTP transport, durable outbox, messages/, templates/
    ├── users/                          controller, service, policy, repository, entity, dto, error
    ├── auth/                           controllers/ (session, password, email, totp), services/, repositories/,
    │                                    dto/, helpers/, entity, error
    ├── avatars/                        profile photos: upload processing, public serving (stored via ObjectStorage)
    ├── media/                          user file uploads: type detection, ownership, private serving (stored via ObjectStorage)
    ├── audit_log/                      admin action log: controller, service, repository, entity, dto
    ├── api_keys/                       hashed, scoped, revocable keys
    ├── oauth/                          Google and GitHub (PKCE, state, one-time exchange code)
    ├── passkeys/                       WebAuthn registration and usernameless sign-in
    └── qr_login/                       QR-code sign-in sessions
migrations/                             0001 users ... 0015 media (applied automatically at startup)
seeds/seed.sql, seeds/reset.sql         development seed data / wipe (`cargo run -- seed [--fresh]`)
tests/                                  integration tests (see Testing)
Dockerfile, .dockerignore               multi-stage build for the API image
.github/workflows/ci.yml                fmt/clippy/test on every push and PR, plus a dependency audit job
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

Modules depend on each other in one direction: `auth`, `avatars`, `api_keys`, `oauth`, `passkeys` and `qr_login` use `users` (and `auth` uses `mail`); `users` and `mail` use only `common`. That is why the JWT, password and `AuthUser`/`SessionUser`/`AdminUser` code lives in `common/security/` rather than in `auth`. API keys are resolved through a small `ApiKeyVerifier` interface defined in `common`, implemented by `api_keys`, so the guards do not depend on that feature; the same pattern (`SessionAuth`/`SessionVerifier`) lets the guards check live session state without depending on `users`.

Axum extractors play the role of guards and pipes: `AuthUser` and `AdminUser` are guards, `ValidatedJson<T>` is the validation pipe, and tower layers in `common/middleware/` are the middleware. Failures are named errors (`AuthError::InvalidCredentials`, `UsersError::EmailTaken`, ...) that each convert into `AppError`, the single place that maps errors to HTTP status codes.

### Adding a feature

1. Create `modules/<name>/` with `mod.rs`, `controller.rs`, `service.rs`, `repository.rs`, `entity.rs`, `dto/`, `error.rs` (`impl From<YourError> for AppError`).
2. Add the migration under `migrations/` (`0016_<name>.sql`).
3. Build the service in `AppState::with_mail` (`state.rs`) and add it as an `Arc<YourService>` field; `#[derive(FromRef)]` lets handlers take `State<Arc<YourService>>`. Keep every `AppState` field a cheap handle (`Arc`, pool): axum clones the state on every request, so a `String` or `Vec` field would be copied each time.
4. Nest its router in `modules/mod.rs`.
5. Annotate handlers with `#[utoipa::path(...)]` and DTOs with `#[derive(ToSchema)]`, then list them in `infra/openapi.rs`'s `ApiDoc` so they show up at `/docs`.

New emails: add a struct in `modules/mail/messages/`, a `<name>.html` and `<name>.txt` in `modules/mail/templates/`, and a `send_<name>` method on `MailService`.

## How the account flows work

Reset, verification and email-change links share one mechanism, the `auth_tokens` table:

- The token is 32 random bytes (base64url) and exists only inside the emailed link. The database stores its SHA-256.
- A token works once (claimed with a single atomic `UPDATE ... RETURNING`) and only before it expires.
- Issuing a new token of the same kind for a user invalidates the older ones.
- `forgot` answers identically for known and unknown addresses, sends the mail in the background, and sends at most 3 reset emails per account per hour.
- Following a reset or invitation link also marks the email verified, since it proves control of the inbox.

Links point at your frontend: `{FRONTEND_URL}/reset-password?token=...`, `/verify-email?token=...`, `/confirm-email-change?token=...`. The frontend page then POSTs the token to the matching endpoint.

### Password policy

Every password (register, reset, change, admin-create) is scored with `zxcvbn` and rejected below a "somewhat guessable" threshold (`common::security::password_policy`) — the account's own email and display name are fed in as "known" context, so e.g. `alice2024` for `alice@example.com` scores lower than it would judged alone. Set `CHECK_PASSWORD_BREACHES=true` to also reject passwords found in the Have I Been Pwned database; an outage of that external service never blocks registration or login (any network failure is treated as "not found").

### Account lockout

Independent of the per-IP rate limiter (which does not slow down a distributed attempt against one account from many IPs), each account tracks its own wrong-password count. After `MAX_FAILED_LOGIN_ATTEMPTS` (default 5) it is locked for `ACCOUNT_LOCKOUT_MINUTES` (default 15) — rejected with the same generic "invalid credentials" message as a wrong password, even for the correct one, so failing a login a few times can never be used to confirm an email is registered. A correct password always resets the counter, even if the sign-in is then rejected for another reason (e.g. an unverified email).

### Sessions and revocation

Every JWT carries the user's `token_version`, checked live against the database on every request (`common::security::session_auth`) — role and active/deleted status are always read fresh from the database too, never trusted from the token. A password change bumps `token_version`; demotion, deactivation and deletion take effect through the live status check. So all four take effect on the very next request, not just once the token expires. This trades a stateless, zero-DB-lookup check for one lookup per authenticated request — the same cost API keys already paid.

## Mail

`MailService` renders each email as text plus HTML and sends it in a background task with up to 3 attempts (1 s and 4 s backoff) for transient SMTP errors. A mail failure is logged (without the link) and never fails a request. Pending mail is given 5 seconds to drain on shutdown.

Every email is also persisted to `outbound_mail` before the send is attempted (`MailService::with_durable_outbox`, wired up for the real service only — never for the in-memory test transport) and removed once it succeeds. A row still `pending` at the next startup means the process crashed between rendering and sending; it is resent automatically in the background as soon as the process starts (concurrently with, not blocking, the server accepting requests). A permanently failed send is kept as `dead` for a month (pruned by the same cleanup job as the job queue) rather than deleted, so a real delivery failure stays inspectable.

To use your SMTP server, set `MAIL_ENABLED=true`, `SMTP_HOST`, `SMTP_PORT`, `SMTP_TLS`, `MAIL_FROM` and, if required, `SMTP_USERNAME`/`SMTP_PASSWORD`. TLS uses the system trust store; for a private CA set `SSL_CERT_FILE`, or use `SMTP_TLS=none` on a private Docker network. For mail to reach inboxes, the sending domain needs SPF, DKIM and DMARC records.

To catch mail locally instead of sending it:

```bash
docker compose -f docker-compose.dev.yml up -d   # Mailpit: SMTP :1025, web UI http://localhost:8025
# .env: MAIL_ENABLED=true  SMTP_HOST=localhost  SMTP_PORT=1025  SMTP_TLS=none
```

## Background jobs

`infra/jobs/` is a small durable job queue backed by Postgres, claimed with `FOR UPDATE SKIP LOCKED` — the same mechanism the `pgmq` extension uses internally, so it needs no extension and works on any Postgres instance. It runs inside the same process as the web server (no separate worker to deploy) and is drained, like mail, for 5 seconds on shutdown.

Today it runs one recurring job, `cleanup_expired_rows`: every hour it sweeps rows that expired over an hour ago from `auth_tokens`, `oauth_states`, `webauthn_challenges`, `qr_sessions` and `login_grants` (nothing did this on a schedule before; it only happened opportunistically on the next insert into the same table), then re-enqueues itself — a cron job with no `pg_cron` needed. It also prunes its own history (`succeeded` rows after a day, `dead` rows after a month), so the `jobs` table does not grow forever.

Add a job kind by matching on it in `infra/jobs/worker.rs::dispatch`, following the shape of `infra/jobs/cleanup.rs`. Failed jobs back off (1s, 4s, 16s, then every 16s) and move to a `dead` status after 5 attempts, kept (not deleted) so they stay inspectable. A custom, minimal, Postgres-native queue was chosen over `apalis` or a Redis/broker-backed one to avoid an extra moving part, consistent with the rest of this starter's hand-rolled pieces (JWT, mail, rate limiter) — appropriate at a starter project's scale; revisit if throughput or multi-service fan-out ever demands a real broker.

## Monitoring

`GET /metrics` (Prometheus text format) is served on its **own listener** (`METRICS_BIND_ADDR`, default `127.0.0.1:9091`) — deliberately outside the main app's router, so it carries none of its CORS/compression/timeout middleware and is not itself counted in `http_requests_total`. Set `METRICS_TOKEN` to require `Authorization: Bearer <token>` on it.

What's exposed:

- **HTTP metrics**, automatic (via `axum-prometheus`): `axum_http_requests_total`, `axum_http_requests_duration_seconds`, `axum_http_requests_pending`, all labelled by `method`, `endpoint` (the route pattern, e.g. `/users/{id}`, not the raw path — safe cardinality) and `status`.
- **Gauges**, sampled every 15s (`infra/metrics.rs::spawn_gauge_sampler`): `db_pool_connections`, `db_pool_idle_connections`, `rate_limiter_tracked_clients`, `password_hash_permits_available`.
- **Business counters**, recorded at their call sites: `auth_register_total`, `auth_login_total{outcome}`, `auth_totp_verify_total{outcome}`, `password_reset_requested_total`, `password_reset_completed_total`, `oauth_login_total{provider,outcome}`, `passkey_ceremony_total{kind,outcome}`, `qr_login_total{outcome}`, `api_key_auth_total{outcome}`, `mail_send_total{outcome}`, `rate_limit_rejections_total{limiter}`.

Local Prometheus + Grafana:

```bash
# METRICS_BIND_ADDR=0.0.0.0:9091 in .env first — a container can't reach 127.0.0.1 on the host
docker compose -f docker-compose.monitoring.yml up -d
# Prometheus: http://localhost:9090   Grafana: http://localhost:3002 (admin/admin, Prometheus datasource pre-provisioned)
```

`METRICS_BIND_ADDR` defaults to loopback-only because `/metrics` has no auth by default; `0.0.0.0` is only for local Docker-based scraping, not for exposing the port on a shared or public network.

## Deployment and CI

- `Dockerfile`: multi-stage build (`cargo build --release` in a `rust:slim` image, running in `debian:bookworm-slim`). Migrations are compiled into the binary, so the runtime image needs nothing from `migrations/`.
- `.github/workflows/ci.yml`: on every push/PR, runs `cargo fmt --check`, `cargo clippy -- -D warnings` and the full test suite against a Postgres service container, plus a separate job auditing dependencies against the RustSec advisory database (`rustsec/audit-check`).

## Testing

```bash
cargo test
cargo clippy --all-targets
```

- Unit tests sit next to the code (token hashing, policy rules, pagination, rate limiter, password strength, TOTP, recovery codes, templates, ...).
- `tests/*.rs` drive the full router in-process against a fresh, throwaway Postgres database created per test (see `tests/common/mod.rs`; needs `TEST_DATABASE_URL` or a local Postgres on `localhost:5432` with the default `postgres`/`postgres` credentials), with an in-memory mail transport so tests can read the emailed links: `auth` (including account lockout), `totp` (full 2FA enroll/login/recovery-code flow), `audit_log`, `users` (including a real concurrent race for the last-admin guard), `avatars`, `media` (upload/type-detection/privacy/cleanup rules), `api_keys`, `oauth` (against a fake Google/GitHub server), `passkeys` (a software authenticator performs the real WebAuthn ceremonies), `qr_login`, `jobs`, `seed`.
- `tests/mail_smtp.rs` runs the real SMTP transport against a small fake SMTP server, including recovering an email left `pending` by a simulated crash.

## Known limits

Deliberate scope boundaries for a starter, not oversights:

- Soft delete rewrites the user's email to `deleted+<id>@deleted.invalid` so the address can be registered again — by design.
- Password hashing is capped at `MAX_CONCURRENT_HASHES` at a time (default 8), so a burst of logins queues instead of allocating 19 MiB each. Under a burst, requests wait for a free slot; the 10 s request timeout still applies.
- The rate limiter is per-replica, and uploads (profile photos and media) default to local disk (`UPLOAD_DIR`), which is single-host. Set `S3_BUCKET` so several API replicas can share them. Sharing the rate limiter across replicas needs an external store (Redis) this starter deliberately does not bundle.
- Media is buffered in memory on upload and download (at most `MEDIA_MAX_UPLOAD_BYTES`, 8 MiB by default), and there is no range-request support, no per-user quota and no virus scanning. That suits images, documents and short clips; large video needs presigned S3 URLs or multipart uploads, which this starter does not include.
- Passkeys are usernameless (discoverable) only. Hardware keys that create non-discoverable credentials cannot sign in, and the passkey flow is verified with a software authenticator in tests, not with every browser and device.
- QR login's residual risk (a user approving a login they did not start) is inherent to the UX pattern itself (the same risk WhatsApp Web has); the verification code and requester display reduce, not remove, it.
- Passkeys need OpenSSL at build and run time (see steps.md) — a transitive dependency of `webauthn-rs`.
