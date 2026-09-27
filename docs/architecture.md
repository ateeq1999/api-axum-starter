# Architecture

How the template is put together, and the rules that keep it that way. Read this once before you
add code; [adding-a-feature.md](adding-a-feature.md) then makes it concrete.

## The shape of a request

```
client
  |
  v
global layers (src/common/middleware/stack.rs)
  request id -> tracing -> 10 s timeout -> compression -> CORS allow-list -> body size limit
  |
  v
router   /health/*   /docs   /api/v1/<module>/...           (src/app.rs, src/modules/mod.rs)
  |
  v
extractors   guard (AuthUser / SessionUser / AdminUser)  +  ValidatedJson / ValidatedQuery / Path
  |            a bad token is a 401, an invalid body is a 422, before your handler runs
  v
controller   unpack the request, call ONE service method, wrap the result
  |
  v
service      the business rules; no HTTP types, no SQL
  |
  v
repository   all the SQL
  |
  v
PostgreSQL

any error, at any step  ->  AppError  ->  { "error": { "code", "message", "details"? } } + status
```

## Layers, one direction only

| Layer | Does | Must not |
|---|---|---|
| **Controller** (`controllers/`, one file per endpoint) | Axum types: extractors in, `Json`/status out. Calls one service method | Contain SQL, hashing, or rules |
| **Service** (`services/`, one file per use case) | Business rules, orchestration, calls other services | Know about axum, headers, or status codes |
| **Repository** (`repository.rs`, or `repositories/` split by table or concern) | SQL, and only SQL. Plain values in, entities out | Decide anything about permissions or rules |
| **DTO** (`dto/`) | Request and response shapes, validation attributes, OpenAPI schemas | Be a database entity |
| **Entity** (`entity.rs`) | One database row (`sqlx::FromRow`) | Be serialized to a client |

Because the service has no web types in its signatures, you can call it from a background job, a
CLI subcommand, or another feature without pulling in axum.

## Where things live

```
src/
├── main.rs, lib.rs, app.rs, state.rs   entry point, module tree, router assembly, AppState
├── cli/                                 subcommands: serve, seed, setup, gen --secrets
├── config/                              environment-driven Config (fails fast at startup)
├── infra/                               database pool + migrations, jobs/ (queue), storage.rs
│                                        (S3 or disk), metrics, telemetry, openapi, secrets, seed
├── common/                              feature-agnostic building blocks
│   ├── error.rs                         AppError -> HTTP response
│   ├── dto/                             Pagination, PaginatedResponse, MessageResponse
│   ├── extractors/                      ValidatedJson, ValidatedQuery, ClientMeta
│   ├── security/                        jwt, passwords, totp, Role, AuthUser/SessionUser/AdminUser
│   └── middleware/                      request id, tracing, rate limiter, the global layer stack
└── modules/                             one folder per feature
    ├── users/  auth/  api_keys/  oauth/  passkeys/  qr_login/
    ├── avatars/  media/  audit_log/  mail/  health/
    └── <your feature>/
migrations/                              0001... applied in order at startup, embedded in the binary
tests/                                   integration tests (one file per feature)
```

**Where does my code go?**

| I am adding... | It goes in |
|---|---|
| A feature with its own tables and endpoints | A new `src/modules/<name>/` ([walkthrough](adding-a-feature.md)) |
| Something several features need (a guard, an extractor, a helper) | `src/common/` |
| A technical service (queue, storage, metrics) with no business meaning | `src/infra/` |
| A setting | `src/config/` ([recipe](recipes.md#add-a-configuration-setting)) |
| An email | `src/modules/mail/` ([recipe](recipes.md#send-an-email)) |
| Recurring or deferred work | `src/infra/jobs/` ([recipe](recipes.md#run-work-in-the-background)) |

## Module dependencies

Modules depend on each other in one direction only:

- Feature modules (`auth`, `avatars`, `api_keys`, `oauth`, `passkeys`, `qr_login`, and yours) may
  use `users`.
- `users` and `mail` use only `common`.
- **`common` never depends on a feature.** When `common` needs something a feature provides, it
  defines a small trait and the feature implements it: API keys are resolved through
  `ApiKeyVerifier`, and live session checks (role, active status, token version) go through
  `SessionVerifier`, both implemented by modules and handed to the guards through `AppState`. That
  is why the JWT, password and guard code lives in `common/security/` and not in `auth`.

If you catch yourself importing one feature from another in both directions, extract the shared
part into `common` or put a trait in `common`.

## `AppState`

`src/state.rs` builds every service once at startup and keeps it in `AppState`. axum clones the
state for every request, so **every field must be a cheap handle** (an `Arc`, a pool, a type that
is an `Arc` inside). A `String` or `Vec` field would be copied on every request.

`AppState` derives `FromRef`, so a handler can ask for exactly what it needs:

```rust
async fn list(actor: AuthUser, State(notes): State<Arc<NotesService>>) -> ... 
```

Services are `Clone` (they hold a pool or other `Arc`s), and are constructed with their
dependencies passed in (`UsersService::new(repo, ..., audit_log)`): no globals, no lazy statics.

## Errors

Each feature has its own error enum named after what went wrong (`UsersError::EmailTaken`,
`NotesError::NotFound`). One `impl From<YourError> for AppError` maps each variant to a status.
`AppError` is the single type that turns into an HTTP response, so the wire format is uniform:

```json
{ "error": { "code": "not_found", "message": "note not found" } }
```

Validation failures add per-field `details`. Internal and database errors are logged with full
detail and returned to the client as a generic `internal_error`, never leaking SQL or paths. The
mapping table is in [endpoints.md](endpoints.md#errors-and-status-codes).

## Security model

- **Authentication** is a bearer token: a JWT from an interactive sign-in (password, OAuth,
  passkey, QR code), or an API key (`X-API-Key: ak_...` or `Authorization: Bearer ak_...`).
- **Sessions are checked live.** Every JWT carries the user's `token_version`, compared against
  the database on every request, and role, active and deleted status are read fresh. So a password
  change, demotion, deactivation or deletion takes effect on the very next request. The cost is
  one lookup per authenticated request.
- **API keys** are `read` (safe HTTP methods only) or `write`, and can never manage credentials.
  This is enforced by the guard types, so a new endpoint gets it without extra code.
- **Passwords** are hashed with argon2 (concurrency-capped), scored with zxcvbn, optionally
  checked against known breaches. Accounts lock temporarily after repeated failures.
- **Rate limiting** protects the unauthenticated endpoints (per client IP).

The guard types and when to use each are in [endpoints.md](endpoints.md#choosing-a-guard).

## Configuration

All configuration is environment variables, loaded from `.env` if present (real environment
variables win). `Config::from_env()` fails fast at startup on a missing or invalid value, and a
**blank value counts as unset**, so `.env.example` can have every line active. The full list is in
the [README's configuration section](../README.md#configuration).

## Data and migrations

- PostgreSQL through `sqlx`, with queries as SQL strings in repositories.
- Migrations are forward-only files in `migrations/`, applied at startup and compiled into the
  binary. Never edit one that has shipped.
- UUID primary keys generated in Rust, `TIMESTAMPTZ` for time, `TEXT` with a `CHECK` where an enum
  is stored.
- Soft delete (users): the row stays with `deleted_at` set and drops out of every query and out of
  email uniqueness (a partial unique index), so the address can be registered again.

## Background work and side effects

- **Email** is rendered from templates and sent by a background task with retries, backed by a
  durable outbox table so a crash between "queued" and "sent" loses nothing.
- **Jobs** run on a small Postgres-backed queue inside the same process as the server.
- **Files** go through one `ObjectStorage` service (local disk, or S3 when `S3_BUCKET` is set).

Each has a recipe in [recipes.md](recipes.md).

## Conventions

- Names say what a thing is or does: `find_owned`, `ensure_room_to_pin`, `MAX_PINNED_NOTES_PER_USER`.
  No abbreviations, nothing vague.
- Comments explain a non-obvious *why* (a hidden constraint, a subtle invariant), never what the
  code plainly says.
- Handlers are `pub(crate) async fn` so `infra/openapi.rs` can list them.
- One integration test file per feature in `tests/`, plus unit tests next to pure logic.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` must pass (CI runs
  the same).
