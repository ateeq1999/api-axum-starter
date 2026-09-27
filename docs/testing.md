# Testing

Integration tests run the **real router in-process against a fresh, throwaway Postgres database
per test**, so they exercise the same code, migrations and SQL as production, with no mocks and no
shared state between tests.

```bash
cargo test                         # everything
cargo test --test notes            # one feature's file
cargo test the_last_active_admin   # by test name
cargo fmt --all && cargo clippy --all-targets -- -D warnings   # what CI also runs
```

**Needs** a Postgres it can create databases in: `TEST_DATABASE_URL`, or a local one on
`localhost:5432` with the default `postgres`/`postgres` credentials. Leftover test databases from
crashed runs are cleaned up automatically.

## Anatomy of a test

```rust
mod common;                                       // tests/common/mod.rs, the harness
use common::spawn;
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn create_a_note() {
    let app = spawn().await;                      // new database, migrated, router built
    let (_id, token) = app.user("me@example.com").await;   // registered + signed in

    let (status, body) = app
        .post("/api/v1/notes", Some(&token), json!({ "title": "Hello" }))
        .await;

    assert_eq!(status, StatusCode::CREATED, "{body}");   // print the body on failure
    assert_eq!(body["title"], "Hello");
}
```

## The harness (`tests/common/mod.rs`)

| Helper | Gives you |
|---|---|
| `spawn().await` | A `TestApp` with a fresh database and the full router |
| `spawn_with(config).await` | The same with a customized `Config` (start from `test_config()`) |
| `app.user(email).await` | `(id, token)` for a registered, signed-in normal user |
| `app.admin(email).await` | `(id, token)` for a user promoted to admin |
| `app.register(email)`, `app.login(email, password)`, `app.login_token(email)` | The individual steps |
| `app.get / post / patch / delete(uri, Some(&token), [json])` | `(StatusCode, serde_json::Value)`; pass `None` for no token |
| `app.raw(method, uri, headers, body)` | A `RawResponse` with `status`, `headers`, `body` bytes, `.json()`. For uploads, downloads and header checks |
| `app.json_with(method, uri, headers, body)` | JSON request with extra headers (`X-API-Key`, ...) |
| `app.mails_to(address).await` | The emails the app "sent" (an in-memory transport records them) |
| `token_from(&mail)` | The `token=` value from a mail's link, to finish reset/verify flows |
| `app.state.db` | The `PgPool`, for arranging or checking rows directly |
| `PASSWORD` | A password that passes the strength rules |

## What to test for an endpoint

The notes tests (`tests/notes.rs`) are a good template. For each new endpoint cover:

1. **The happy path**, checking the response body, not only the status.
2. **Validation:** an invalid body gives `422`, `code == "validation_failed"`, and `details` names
   the field.
3. **Someone else's data:** another user gets `404` for read, edit **and** delete, and the owner's
   data is unchanged afterwards.
4. **No token:** `401`.
5. **The wrong role:** a normal user on an admin route gets `403`.
6. **Your rules:** every limit and branch in the service (the pin cap, "already exists", ...).
7. **Side effects:** an email was sent (`mails_to`), a row was written or removed (`app.state.db`),
   a file was stored.

Arrange state directly in SQL when going through the API would be slow or impossible (aging a row
by two years, promoting a user):

```rust
sqlx::query("UPDATE notes SET updated_at = now() - interval '2 years' WHERE owner_id = $1::uuid")
    .bind(&owner_id)
    .execute(&app.state.db)
    .await
    .unwrap();
```

## Patterns

**API-key behavior.** Create a key with a session token, then call with `X-API-Key`. A read-only
key can `GET` and gets `403` on `POST`, with no code in your feature (see the last notes test).

**Concurrency.** To prove an invariant holds under a race, fire the requests together with
`tokio::join!` and assert on the outcome, as `the_last_active_admin_cannot_be_removed` does. Bind
the request paths to variables first so the temporaries live long enough.

**Emails.** Assert on `subject`, `text` and `html` from `mails_to`. To complete a flow that emails a
token, take it with `token_from(&mail)` and post it to the confirm endpoint.

**Jobs.** Call the job's `run` function against the test database and assert on rows. The queue
itself is covered by `tests/jobs.rs`.

**Files and S3.** File tests use local disk by default. Tests that need real S3 are skipped unless
you point them at one (a local emulator such as floci works):

```bash
export AWS_ENDPOINT_URL=http://localhost:4566 AWS_DEFAULT_REGION=us-east-1 \
       AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test
TEST_S3_BUCKET=my-bucket cargo test --test media --test avatars
```

**Pure logic.** Put rules that need no database in small pure functions with `#[cfg(test)]`
unit tests next to them (`modules/users/policy.rs`, `common/net.rs`, the password policy). They
run in milliseconds and pin down the edge cases.

## Adding a config field breaks the harness build

`tests/common/mod.rs::test_config()` builds a `Config` by hand, so a new field makes every
integration test fail to compile until you add it there. That is deliberate: it makes you decide
what the tests should use.
