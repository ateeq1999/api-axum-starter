# Adding and changing API endpoints

A reference for everything you decide when you write a handler. For a full feature from scratch,
follow [adding-a-feature.md](adding-a-feature.md); this page is what you come back to for one
endpoint.

- [Anatomy of a handler](#anatomy-of-a-handler)
- [Choosing a guard](#choosing-a-guard)
- [Reading the request](#reading-the-request)
- [Returning a response](#returning-a-response)
- [Validation](#validation)
- [Errors and status codes](#errors-and-status-codes)
- [Ownership and permissions](#ownership-and-permissions)
- [Routing and public endpoints](#routing-and-public-endpoints)
- [Rate limiting a route](#rate-limiting-a-route)
- [Pagination and search](#pagination-and-search)
- [Documenting it in Swagger](#documenting-it-in-swagger)
- [Limits worth knowing](#limits-worth-knowing)

## Anatomy of a handler

```rust
#[utoipa::path(
    patch,
    path = "/api/v1/notes/{id}",
    params(("id" = Uuid, Path, description = "Note id")),
    request_body = UpdateNoteDto,
    responses(
        (status = 200, description = "Updated note", body = NoteResponse),
        (status = 404, description = "No such note, or it is not yours"),
    ),
    security(("bearer_auth" = [])),
    tag = "notes"
)]
pub(crate) async fn update(
    actor: AuthUser,                                   // who is calling (the guard)
    State(notes): State<Arc<NotesService>>,            // the service, from AppState
    Path(id): Path<Uuid>,                              // the {id} in the URL
    ValidatedJson(dto): ValidatedJson<UpdateNoteDto>,  // the JSON body, validated
) -> AppResult<Json<NoteResponse>> {
    Ok(Json(notes.update(&actor, id, dto).await?))     // ONE service call
}
```

- **Order matters for the body:** the request body can be consumed once, so `ValidatedJson`
  (or a raw `Request`) must be the **last** parameter.
- **Return `AppResult<T>`:** any `?` on a service call turns errors into the right HTTP response.
- **`pub(crate)`** so `src/infra/openapi.rs` can reference the function.

## Choosing a guard

The guard is the first parameter (or any parameter) of type `AuthUser`, `SessionUser` or
`AdminUser`. **A handler with none of them is public.**

| Guard | Accepts | Rejects | Use it for |
|---|---|---|---|
| `AuthUser` | A signed-in user's JWT, or an API key | Missing/invalid token (401); a **read-only API key on an unsafe method** (403) | Ordinary data endpoints. Gives you `actor.id` and `actor.role` |
| `SessionUser(actor)` | Interactive sign-in (JWT) only | API keys (403) | Anything that manages credentials: passwords, emails, API keys, passkeys, linked accounts, approving logins. A leaked API key can never take an account over |
| `AdminUser(actor)` | Admins (JWT or a write-scoped admin key) | Non-admins (403) | Admin-only endpoints |
| none | Anyone | | Health checks, sign-in, registration, public files |

What the guards do for you, automatically and on every request:

- Check the token (signature and expiry), then **check the account live against the database**:
  still exists, still active, role read fresh, `token_version` matches. A deactivated user's old
  token stops working immediately.
- For API keys, look the key up by hash, check expiry and revocation, and enforce the
  `read`/`write` scope by HTTP method (`GET`/`HEAD`/`OPTIONS` are safe; the rest need `write`).

`AuthUser` gives you:

```rust
actor.id          // Uuid of the account
actor.role        // Role::User | Role::Admin, has .is_admin()
actor.credential  // Credential::Session | Credential::ApiKey(scope)
```

## Reading the request

| You need | Use |
|---|---|
| A JSON body | `ValidatedJson<Dto>` (rejects bad JSON with 400 and failed rules with 422) |
| Query parameters | `ValidatedQuery<Query>`; derive `IntoParams` for Swagger |
| A path parameter | `Path<Uuid>` (route syntax is `"/{id}"`) |
| A service | `State<Arc<YourService>>` |
| The caller's IP and user agent | `ClientMeta` (`.ip`, `.user_agent`; respects `TRUST_PROXY_HEADERS`) |
| Raw bytes (an upload) | `request: axum::extract::Request`, read with `to_bytes(request.into_body(), limit)`. See `modules/media/controllers/upload_media.rs` |

For a raw upload, check `Content-Length` against your limit first and cap `to_bytes`, as the media
controller does, so an oversized body is rejected as `413` before it is buffered.

## Returning a response

| Situation | Return |
|---|---|
| A resource | `Json(dto)` |
| Created | `(StatusCode::CREATED, Json(dto))` |
| Nothing to return | `StatusCode::NO_CONTENT` (204) |
| Just a message | `Json(MessageResponse::new("Password updated."))` |
| A page of results | `Json(PaginatedResponse<T>)`, shaped `{ items, page, per_page, total }` |
| Non-JSON (a file) | Build a `Response`; set `Content-Type`, and think about `Content-Disposition`, `nosniff` and caching (see `modules/media/controllers/download_content.rs`) |

Never return an entity; map it to a response DTO (`impl From<Entity> for Response`).

## Validation

Put rules on the DTO fields. The extractor runs them before your handler.

```rust
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateNoteDto {
    #[validate(length(min = 1, max = 200, message = "must be 1-200 characters"))]
    pub title: String,
    #[validate(length(max = 20000, message = "must be at most 20000 characters"))]
    pub body: Option<String>,
}
```

A failure is `422` with per-field details the frontend can attach to inputs:

```json
{
  "error": {
    "code": "validation_failed",
    "message": "validation failed",
    "details": { "title": [ { "code": "length", "message": "must be 1-200 characters", "params": { "min": 1, "max": 200 } } ] }
  }
}
```

Rules that need the database or other state ("at most 5 pinned", "email not taken") belong in the
**service**, returned as a feature error, not in the DTO.

## Errors and status codes

Return a feature error from the service; its `From` impl picks the variant.

| `AppError` variant | Status | `code` | Use when |
|---|---|---|---|
| `BadRequest(msg)` | 400 | `bad_request` | The request is well-formed but a rule rejects it (pin limit, wrong current password) |
| `Unauthorized(msg)` | 401 | `unauthorized` | Not authenticated, or wrong credentials |
| `Forbidden(msg)` | 403 | `forbidden` | Authenticated but not allowed (admin only, read-only key) |
| `NotFound` | 404 | `not_found` | Missing, **or not yours** (do not leak existence) |
| `Conflict(msg)` | 409 | `conflict` | Would violate uniqueness (email already registered) |
| `PayloadTooLarge(msg)` | 413 | `payload_too_large` | Body over a limit |
| `UnsupportedMediaType(msg)` | 415 | `unsupported_media_type` | File type not accepted |
| `Validation(...)` | 422 | `validation_failed` | Produced by the extractors, with `details` |
| `TooManyRequests` | 429 | `too_many_requests` | Produced by the rate limiter |
| `Internal(..)` / `Database(..)` | 500 | `internal_error` | Anything unexpected. Logged in full, returned as a generic message |

Adding an error to a feature is three lines in its `error.rs`:

```rust
#[error("you can have at most {0} pinned notes, unpin one first")]
TooManyPinned(i64),                       // the variant, with the message
// ...and in the From impl:
NotesError::TooManyPinned(_) => AppError::BadRequest(message),
```

`sqlx` errors convert to `AppError::Database` with `?`. To turn a specific database error into a
friendly one, match it in the repository, as users does for a duplicate email
(`sqlx::Error::Database(e) if e.is_unique_violation()` becomes `EmailTaken`).

## Ownership and permissions

Three patterns, from simplest to most general:

1. **Owner-scoped in SQL** (default for user data): `WHERE id = $1 AND owner_id = $2`. Someone else's
   row is simply absent, so the answer is `404`. See `find_owned` in the notes repository.
2. **Role check with the guard:** take `AdminUser` instead of `AuthUser`.
3. **Rules that depend on who is acting on whom:** write small pure functions and unit-test them,
   as `modules/users/policy.rs` does (`can_view`, `check_delete`, ...). The service calls them
   before acting. Keeping them pure means they are testable without HTTP or a database.

"Admin or the owner" is the common combination:

```rust
let can_see = actor.role.is_admin() || row.owner_id == actor.id;
```

Roles are the `Role` enum (`User`, `Admin`) stored as text. Adding a role means a new variant, a
migration if you constrain the column, and updating the places that match on it.

## Routing and public endpoints

A feature's router uses **relative** paths; `src/modules/mod.rs` nests it and the whole tree is
served under `/api/v1`:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_one).patch(update).delete(remove))
}
// modules/mod.rs:  .nest("/notes", notes::router())
```

For a **public** endpoint, write a handler with no guard extractor. For a feature that mixes public
and authenticated routes, split the router by kind (the auth module does: `rate_limited()` and
`authenticated()` groups, merged in `controllers/mod.rs`). To add a whole API version, nest
another router next to `/api/v1` in `src/app.rs`.

## Rate limiting a route

Unauthenticated endpoints that an attacker can hammer (sign-in, registration, "forgot password")
are limited per client IP. Wrap the routes in a `RateLimiter` layer:

```rust
use axum::middleware;
use crate::common::middleware::rate_limit::{self, RateLimiter};

pub fn router(limiter: RateLimiter) -> Router<AppState> {
    let limited = Router::new()
        .route("/subscribe", post(subscribe))
        .route_layer(middleware::from_fn_with_state(limiter, rate_limit::enforce));

    limited.merge(Router::new().route("/", get(list)))   // the rest is not limited
}
// modules/mod.rs: pass the shared limiter, `state.rate_limiter.clone()` (RATE_LIMIT_PER_MINUTE)
```

For a different budget make your own:
`RateLimiter::new("name", max_requests, Duration::from_secs(60), trust_proxy_headers)` (the QR
polling endpoint does). The limiter is in memory and per replica; see the README's known limits.

## Pagination and search

`Pagination::new(page, per_page)` clamps to sane values (page >= 1, `per_page` 1-100, default 20)
and gives `limit()` and `offset()`. The repository returns `(items, total)`; the service wraps it:

```rust
let pagination = query.pagination();
let (rows, total) = self.repo.list_owned(actor.id, search, pagination.limit(), pagination.offset()).await?;
Ok(PaginatedResponse::new(rows.into_iter().map(NoteResponse::from).collect(), pagination, total))
```

Always give a list a **stable** order with a tie-breaker (`ORDER BY created_at DESC, id DESC`),
or pages can repeat or skip rows. For `ILIKE` search, escape `%` and `_` in the term (see the
notes repository).

## Documenting it in Swagger

Swagger UI is served at `/docs` and the raw spec at `/api-docs/openapi.json`, generated from your
code by `utoipa`. For a new endpoint:

1. `#[utoipa::path(...)]` on the handler: method, `path` (full, with `/api/v1`), `params`,
   `request_body`, every `responses(...)` status, `security(("bearer_auth" = []))` for guarded
   routes (omit it for public ones), and a `tag`.
2. `#[derive(ToSchema)]` on request and response DTOs; `#[derive(IntoParams)]` with
   `#[into_params(parameter_in = Query)]` on query structs.
3. List the handler under `paths(...)`, the schemas under `components(schemas(...))` and the tag
   under `tags(...)` in `src/infra/openapi.rs`. Generic responses are listed with their type
   argument (`PaginatedResponse<NoteResponse>`).

Types you do not own (for example WebAuthn payloads) are documented as opaque objects.

## Limits worth knowing

- **Request timeout: 10 seconds** for every route. Long work belongs in a
  [background job](recipes.md#run-work-in-the-background).
- **Request body cap: `MAX_REQUEST_BODY_BYTES`** (10 MiB default), enforced before your handler. A
  larger per-route limit needs the global one raised too.
- **CORS** allows only `CORS_ALLOWED_ORIGINS` (default: `FRONTEND_URL`). A browser frontend on
  another origin must be listed.
- **Password hashing is concurrency-capped** (`MAX_CONCURRENT_HASHES`). Do not hash on the hot path
  of a new endpoint; go through the existing password services.
