# Adding a feature: a worked example

This walks through adding a complete feature, **notes** (each user creates, lists, searches, edits
and deletes their own notes), from an empty folder to tested, documented endpoints. Every file below is real code
that compiles and passes its tests against this template. Copy it, rename `note` to whatever your
feature is, and change the rules.

Read [architecture.md](architecture.md) first if you have not: it explains the layers this
example follows (`controller -> service -> repository -> database`).

**What you will add**

| Endpoint | Who | Does |
|---|---|---|
| `POST /api/v1/notes` | signed-in user or write API key | Create a note |
| `GET /api/v1/notes?q=&page=&per_page=` | signed-in user or any API key | List your notes, pinned first, with search |
| `GET /api/v1/notes/{id}` | owner | Read one |
| `PATCH /api/v1/notes/{id}` | owner | Change some fields |
| `DELETE /api/v1/notes/{id}` | owner | Delete |

**Rules it enforces:** you only ever see your own notes (someone else's id is a `404`, not a
`403`, so existence is not leaked); a user can pin at most 5 notes; titles are 1-200 characters.

**The files you touch**

```
migrations/0017_notes.sql                 new   the table
src/modules/notes/                        new   the feature (7 small files)
src/modules/mod.rs                        edit  mount the routes
src/state.rs                              edit  build the service
src/infra/openapi.rs                      edit  list it in Swagger
tests/notes.rs                            new   integration tests
```

## 1. Decide before you type

Answer these; each one maps to a decision below.

| Question | Notes example | Where it lands |
|---|---|---|
| Who may call it? | Any signed-in user, API keys included | The guard type on each handler (step 9) |
| Whose data is it? | The caller's own | Every query filters by `owner_id` (step 6) |
| What can go wrong for the caller? | Not found, too many pinned | An error enum (step 5) |
| What is the shape on the wire? | Title, body, pinned flag | DTOs (step 7) |
| What must be true in the database? | An owner, a title, timestamps | The migration (step 2) |

## 2. The migration

Migrations live in `migrations/`, are numbered in order, are applied automatically at startup, and
are compiled into the binary (`sqlx::migrate!`), so the Docker image needs nothing at runtime. Never edit one
that has shipped; add a new one. Take the next number (this template ships 0001-0016).

**migrations/0017_notes.sql**

```sql
CREATE TABLE notes (
    id         UUID PRIMARY KEY,
    owner_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    body       TEXT NOT NULL DEFAULT '',
    pinned     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_notes_owner_created ON notes (owner_id, created_at DESC);
```

`ON DELETE CASCADE` means a note disappears with its owner's row. The index serves the "my notes,
newest first" query.

## 3. The module skeleton

A feature is one folder under `src/modules/` that owns everything about it. The folder names the
feature; each file name names its role.

**src/modules/notes/mod.rs**

```rust
//! Private notes: each user creates, lists, edits and deletes their own.

pub mod controller;
pub mod dto;
pub mod entity;
pub mod error;
pub mod repository;
pub mod service;

pub use controller::router;
pub use error::NotesError;
pub use repository::NotesRepository;
pub use service::NotesService;
```

## 4. The entity

The entity mirrors one database row and derives `sqlx::FromRow`. It is **never serialized to a
client**: responses go through a DTO (step 7), so adding a column can never leak by accident.

**src/modules/notes/entity.rs**

```rust
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// A database row. Never serialized directly: responses go through a DTO.
#[derive(Debug, Clone, FromRow)]
pub struct Note {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

## 5. The errors

One enum per feature, named after what went wrong from the caller's point of view. The `From`
impl is the only place that decides the HTTP status; everything else just returns `NotesError::...`.
See [endpoints.md](endpoints.md#errors-and-status-codes) for the full mapping table.

**src/modules/notes/error.rs**

```rust
use crate::common::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum NotesError {
    #[error("note not found")]
    NotFound,
    #[error("you can have at most {0} pinned notes, unpin one first")]
    TooManyPinned(i64),
}

impl From<NotesError> for AppError {
    fn from(e: NotesError) -> Self {
        let message = e.to_string();
        match e {
            NotesError::NotFound => AppError::NotFound,
            NotesError::TooManyPinned(_) => AppError::BadRequest(message),
        }
    }
}
```

## 6. The repository

**All SQL lives here and nowhere else.** Methods take plain values and return entities or counts.
Two habits worth copying:

- The owner check is *in the query* (`WHERE id = $1 AND owner_id = $2`), so a bug in a caller cannot
  read someone else's row.
- `UPDATE ... RETURNING` and `COALESCE($1, title)` give an atomic partial update in one statement,
  with no read-modify-write race.

**src/modules/notes/repository.rs**

```rust
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::entity::Note;
use crate::common::error::AppResult;

/// One place that lists the columns, so a new column is a one-line change.
macro_rules! columns {
    () => {
        "id, owner_id, title, body, pinned, created_at, updated_at"
    };
}

pub struct NewNote<'a> {
    pub owner_id: Uuid,
    pub title: &'a str,
    pub body: &'a str,
    pub pinned: bool,
}

/// `None` leaves a field as it is.
pub struct NotePatch<'a> {
    pub title: Option<&'a str>,
    pub body: Option<&'a str>,
    pub pinned: Option<bool>,
}

/// All the SQL for notes, and nothing else.
#[derive(Clone)]
pub struct NotesRepository {
    db: PgPool,
}

impl NotesRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn insert(&self, new: NewNote<'_>) -> AppResult<Note> {
        let now = Utc::now();
        Ok(sqlx::query_as::<_, Note>(concat!(
            "INSERT INTO notes (id, owner_id, title, body, pinned, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $6)
             RETURNING ",
            columns!()
        ))
        .bind(Uuid::new_v4())
        .bind(new.owner_id)
        .bind(new.title)
        .bind(new.body)
        .bind(new.pinned)
        .bind(now)
        .fetch_one(&self.db)
        .await?)
    }

    /// Scoped to the owner in SQL, so a note that is not yours simply does not exist.
    pub async fn find_owned(&self, owner_id: Uuid, id: Uuid) -> AppResult<Option<Note>> {
        Ok(sqlx::query_as::<_, Note>(concat!(
            "SELECT ",
            columns!(),
            " FROM notes WHERE id = $1 AND owner_id = $2"
        ))
        .bind(id)
        .bind(owner_id)
        .fetch_optional(&self.db)
        .await?)
    }

    /// Pinned first, then newest first. `search` matches title or body, case-insensitively.
    pub async fn list_owned(
        &self,
        owner_id: Uuid,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Note>, u64)> {
        let pattern =
            search.map(|term| format!("%{}%", term.replace('%', "\\%").replace('_', "\\_")));

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notes
             WHERE owner_id = $1 AND ($2::text IS NULL OR title ILIKE $2 OR body ILIKE $2)",
        )
        .bind(owner_id)
        .bind(&pattern)
        .fetch_one(&self.db)
        .await?;

        let notes = sqlx::query_as::<_, Note>(concat!(
            "SELECT ",
            columns!(),
            " FROM notes
             WHERE owner_id = $1 AND ($2::text IS NULL OR title ILIKE $2 OR body ILIKE $2)
             ORDER BY pinned DESC, created_at DESC, id DESC
             LIMIT $3 OFFSET $4"
        ))
        .bind(owner_id)
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        Ok((notes, total.max(0) as u64))
    }

    pub async fn count_pinned(&self, owner_id: Uuid) -> AppResult<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM notes WHERE owner_id = $1 AND pinned")
                .bind(owner_id)
                .fetch_one(&self.db)
                .await?,
        )
    }

    pub async fn update(
        &self,
        owner_id: Uuid,
        id: Uuid,
        patch: NotePatch<'_>,
    ) -> AppResult<Option<Note>> {
        Ok(sqlx::query_as::<_, Note>(concat!(
            "UPDATE notes SET
                 title = COALESCE($1, title),
                 body = COALESCE($2, body),
                 pinned = COALESCE($3, pinned),
                 updated_at = $4
             WHERE id = $5 AND owner_id = $6
             RETURNING ",
            columns!()
        ))
        .bind(patch.title)
        .bind(patch.body)
        .bind(patch.pinned)
        .bind(Utc::now())
        .bind(id)
        .bind(owner_id)
        .fetch_optional(&self.db)
        .await?)
    }

    /// Returns whether a row was deleted.
    pub async fn delete(&self, owner_id: Uuid, id: Uuid) -> AppResult<bool> {
        let result = sqlx::query("DELETE FROM notes WHERE id = $1 AND owner_id = $2")
            .bind(id)
            .bind(owner_id)
            .execute(&self.db)
            .await?;
        Ok(result.rows_affected() == 1)
    }
}
```

## 7. The DTOs

DTOs are the request and response shapes. Validation rules sit right on the fields with
`#[validate(...)]`; `ToSchema` / `IntoParams` feed Swagger. Put each in its own file under `dto/`
and re-export from `dto/mod.rs`.

**src/modules/notes/dto/mod.rs**

```rust
pub mod create_note;
pub mod list_notes_query;
pub mod note_response;
pub mod update_note;

pub use create_note::CreateNoteDto;
pub use list_notes_query::ListNotesQuery;
pub use note_response::NoteResponse;
pub use update_note::UpdateNoteDto;
```

**src/modules/notes/dto/create_note.rs**

```rust
use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateNoteDto {
    #[validate(length(min = 1, max = 200, message = "must be 1-200 characters"))]
    pub title: String,
    #[validate(length(max = 20000, message = "must be at most 20000 characters"))]
    pub body: Option<String>,
    pub pinned: Option<bool>,
}
```

**src/modules/notes/dto/update_note.rs**

```rust
use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

/// Every field is optional: send only what changes.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateNoteDto {
    #[validate(length(min = 1, max = 200, message = "must be 1-200 characters"))]
    pub title: Option<String>,
    #[validate(length(max = 20000, message = "must be at most 20000 characters"))]
    pub body: Option<String>,
    pub pinned: Option<bool>,
}
```

**src/modules/notes/dto/list_notes_query.rs**

```rust
use serde::Deserialize;
use utoipa::IntoParams;
use validator::Validate;

use crate::common::dto::Pagination;

#[derive(Debug, Deserialize, Validate, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListNotesQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    /// Matches title or body, case-insensitively.
    #[validate(length(max = 100, message = "must be at most 100 characters"))]
    pub q: Option<String>,
}

impl ListNotesQuery {
    pub fn pagination(&self) -> Pagination {
        Pagination::new(self.page, self.per_page)
    }
}
```

**src/modules/notes/dto/note_response.rs**

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::modules::notes::entity::Note;

/// What the API returns. Deliberately not the entity: adding a column never leaks it by accident.
#[derive(Debug, Serialize, ToSchema)]
pub struct NoteResponse {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Note> for NoteResponse {
    fn from(note: Note) -> Self {
        Self {
            id: note.id,
            title: note.title,
            body: note.body,
            pinned: note.pinned,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}
```

## 8. The service

The service holds the business rules and knows nothing about HTTP or SQL. It receives the
authenticated caller (`&AuthUser`) and the DTO, applies the rules, and calls the repository.
This is where "at most 5 pinned notes" lives. It is also the layer you would call from a background
job or another module, because it has no web types in its signatures.

**src/modules/notes/service.rs**

```rust
use uuid::Uuid;

use super::{
    dto::{CreateNoteDto, ListNotesQuery, NoteResponse, UpdateNoteDto},
    error::NotesError,
    repository::{NewNote, NotePatch, NotesRepository},
};
use crate::common::{dto::PaginatedResponse, error::AppResult, security::AuthUser};

const MAX_PINNED_NOTES_PER_USER: i64 = 5;

/// The business rules. Knows nothing about HTTP: it takes DTOs and the authenticated caller,
/// and returns responses or errors.
#[derive(Clone)]
pub struct NotesService {
    repo: NotesRepository,
}

impl NotesService {
    pub fn new(repo: NotesRepository) -> Self {
        Self { repo }
    }

    pub async fn create(&self, actor: &AuthUser, dto: CreateNoteDto) -> AppResult<NoteResponse> {
        let pinned = dto.pinned.unwrap_or(false);
        if pinned {
            self.ensure_room_to_pin(actor.id).await?;
        }
        let note = self
            .repo
            .insert(NewNote {
                owner_id: actor.id,
                title: dto.title.trim(),
                body: dto.body.as_deref().unwrap_or(""),
                pinned,
            })
            .await?;
        Ok(note.into())
    }

    pub async fn list(
        &self,
        actor: &AuthUser,
        query: &ListNotesQuery,
    ) -> AppResult<PaginatedResponse<NoteResponse>> {
        let pagination = query.pagination();
        let search = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
        let (notes, total) = self
            .repo
            .list_owned(actor.id, search, pagination.limit(), pagination.offset())
            .await?;
        Ok(PaginatedResponse::new(
            notes.into_iter().map(NoteResponse::from).collect(),
            pagination,
            total,
        ))
    }

    pub async fn get(&self, actor: &AuthUser, id: Uuid) -> AppResult<NoteResponse> {
        let note = self
            .repo
            .find_owned(actor.id, id)
            .await?
            .ok_or(NotesError::NotFound)?;
        Ok(note.into())
    }

    pub async fn update(
        &self,
        actor: &AuthUser,
        id: Uuid,
        dto: UpdateNoteDto,
    ) -> AppResult<NoteResponse> {
        let current = self
            .repo
            .find_owned(actor.id, id)
            .await?
            .ok_or(NotesError::NotFound)?;
        // Only a change from unpinned to pinned needs room.
        if dto.pinned == Some(true) && !current.pinned {
            self.ensure_room_to_pin(actor.id).await?;
        }
        let updated = self
            .repo
            .update(
                actor.id,
                id,
                NotePatch {
                    title: dto.title.as_deref().map(str::trim),
                    body: dto.body.as_deref(),
                    pinned: dto.pinned,
                },
            )
            .await?
            .ok_or(NotesError::NotFound)?;
        Ok(updated.into())
    }

    pub async fn delete(&self, actor: &AuthUser, id: Uuid) -> AppResult<()> {
        if self.repo.delete(actor.id, id).await? {
            Ok(())
        } else {
            Err(NotesError::NotFound.into())
        }
    }

    async fn ensure_room_to_pin(&self, owner_id: Uuid) -> AppResult<()> {
        if self.repo.count_pinned(owner_id).await? >= MAX_PINNED_NOTES_PER_USER {
            return Err(NotesError::TooManyPinned(MAX_PINNED_NOTES_PER_USER).into());
        }
        Ok(())
    }
}
```

## 9. The controller

A controller does three things: unpack the request (the extractors in the signature do most of it),
call **one** service method, wrap the result. No SQL, no rules. The `#[utoipa::path]` block is what
puts the endpoint in Swagger UI, and the handler must be at least `pub(crate)` so `openapi.rs` can
name it.

Choosing the guard is the security decision. `AuthUser` accepts a signed-in user *or* an API key,
and refuses unsafe methods (`POST`, `PATCH`, `DELETE`) for read-only keys automatically. For
anything that manages credentials, use `SessionUser` instead; for admin-only routes, `AdminUser`.
See [endpoints.md](endpoints.md#choosing-a-guard).

**src/modules/notes/controller.rs**

```rust
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use uuid::Uuid;

use super::{
    dto::{CreateNoteDto, ListNotesQuery, NoteResponse, UpdateNoteDto},
    service::NotesService,
};
use crate::{
    common::{
        dto::PaginatedResponse,
        error::AppResult,
        extractors::{ValidatedJson, ValidatedQuery},
        security::AuthUser,
    },
    state::AppState,
};

/// Routes are relative: `modules/mod.rs` nests them under `/notes`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_one).patch(update).delete(remove))
}

// A controller only unpacks the request, calls ONE service method, and wraps the result. No SQL,
// no business rules. The `#[utoipa::path]` block is what puts the endpoint in Swagger UI.

#[utoipa::path(
    post,
    path = "/api/v1/notes",
    request_body = CreateNoteDto,
    responses(
        (status = 201, description = "Note created", body = NoteResponse),
        (status = 400, description = "Too many pinned notes"),
        (status = 422, description = "Validation failed"),
    ),
    security(("bearer_auth" = [])),
    tag = "notes"
)]
pub(crate) async fn create(
    actor: AuthUser,
    State(notes): State<Arc<NotesService>>,
    ValidatedJson(dto): ValidatedJson<CreateNoteDto>,
) -> AppResult<(StatusCode, Json<NoteResponse>)> {
    Ok((StatusCode::CREATED, Json(notes.create(&actor, dto).await?)))
}

#[utoipa::path(
    get,
    path = "/api/v1/notes",
    params(ListNotesQuery),
    responses((status = 200, description = "Your notes, pinned first", body = PaginatedResponse<NoteResponse>)),
    security(("bearer_auth" = [])),
    tag = "notes"
)]
pub(crate) async fn list(
    actor: AuthUser,
    State(notes): State<Arc<NotesService>>,
    ValidatedQuery(query): ValidatedQuery<ListNotesQuery>,
) -> AppResult<Json<PaginatedResponse<NoteResponse>>> {
    Ok(Json(notes.list(&actor, &query).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/notes/{id}",
    params(("id" = Uuid, Path, description = "Note id")),
    responses(
        (status = 200, description = "The note", body = NoteResponse),
        (status = 404, description = "No such note, or it is not yours"),
    ),
    security(("bearer_auth" = [])),
    tag = "notes"
)]
pub(crate) async fn get_one(
    actor: AuthUser,
    State(notes): State<Arc<NotesService>>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<NoteResponse>> {
    Ok(Json(notes.get(&actor, id).await?))
}

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
    actor: AuthUser,
    State(notes): State<Arc<NotesService>>,
    Path(id): Path<Uuid>,
    ValidatedJson(dto): ValidatedJson<UpdateNoteDto>,
) -> AppResult<Json<NoteResponse>> {
    Ok(Json(notes.update(&actor, id, dto).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/notes/{id}",
    params(("id" = Uuid, Path, description = "Note id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "No such note, or it is not yours"),
    ),
    security(("bearer_auth" = [])),
    tag = "notes"
)]
pub(crate) async fn remove(
    actor: AuthUser,
    State(notes): State<Arc<NotesService>>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    notes.delete(&actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

## 10. Wire it in

Three small edits connect the module to the running app.

**src/modules/mod.rs**: declare the module and mount its routes under `/notes` (the whole tree is
served under `/api/v1`).

```rust
pub mod notes;            // next to the other `pub mod` lines

// inside `pub fn router(state: &AppState)`, next to the other `.nest(...)` calls:
        .nest("/notes", notes::router())
```

**src/state.rs**: build the service once at startup and keep it in `AppState` as an `Arc`. Because
`AppState` derives `FromRef`, any handler can now ask for `State<Arc<NotesService>>`.

```rust
// imports
        notes::{NotesRepository, NotesService},

// the struct: a cheap-to-clone handle, never a String or Vec
    pub notes: Arc<NotesService>,

// in `with_mail`, next to the other services
        let notes = NotesService::new(NotesRepository::new(db.clone()));

// in the returned `Self { ... }`
            notes: Arc::new(notes),
```

**src/infra/openapi.rs**: list the handlers, schemas and a tag so they show up at `/docs`.

```rust
// in `paths(...)`
        crate::modules::notes::controller::create,
        crate::modules::notes::controller::list,
        crate::modules::notes::controller::get_one,
        crate::modules::notes::controller::update,
        crate::modules::notes::controller::remove,

// in `components(schemas(...))`; generic responses are listed with their type argument
        crate::modules::notes::dto::CreateNoteDto,
        crate::modules::notes::dto::UpdateNoteDto,
        crate::modules::notes::dto::NoteResponse,
        crate::common::dto::PaginatedResponse<crate::modules::notes::dto::NoteResponse>,

// in `tags(...)`
        (name = "notes", description = "Private notes: create, list, edit, delete your own"),
```

## 11. Test it

Integration tests drive the real router in-process against a fresh throwaway Postgres database
per test. The helpers (`spawn`, `app.user`, `app.get`/`post`/`patch`/`delete`) are described in
[testing.md](testing.md). This file covers the happy path, privacy, validation, the pin cap,
search and pagination, and the API-key behavior you get for free from `AuthUser`. The last test
exercises the recurring job described at the end of this page; drop it if you skip the job.

**tests/notes.rs**

```rust
mod common;

use axum::http::{Method, StatusCode};
use common::spawn;
use serde_json::json;

#[tokio::test]
async fn create_read_update_and_delete_a_note() {
    let app = spawn().await;
    let (_, token) = app.user("writer@example.com").await;

    let (status, created) = app
        .post(
            "/api/v1/notes",
            Some(&token),
            json!({ "title": "  Groceries  ", "body": "milk, eggs" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["title"], "Groceries", "titles are trimmed");
    assert_eq!(created["pinned"], false);
    let id = created["id"].as_str().unwrap().to_string();

    let (status, fetched) = app.get(&format!("/api/v1/notes/{id}"), Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["body"], "milk, eggs");

    // a partial update changes only what was sent
    let (status, updated) = app
        .patch(
            &format!("/api/v1/notes/{id}"),
            Some(&token),
            json!({ "pinned": true }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["pinned"], true);
    assert_eq!(updated["title"], "Groceries");

    let (status, _) = app
        .delete(&format!("/api/v1/notes/{id}"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = app.get(&format!("/api/v1/notes/{id}"), Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn notes_are_private_and_need_a_sign_in() {
    let app = spawn().await;
    let (_, owner) = app.user("owner@example.com").await;
    let (_, stranger) = app.user("stranger@example.com").await;

    let (_, created) = app
        .post("/api/v1/notes", Some(&owner), json!({ "title": "Mine" }))
        .await;
    let uri = format!("/api/v1/notes/{}", created["id"].as_str().unwrap());

    // another user sees a 404, exactly as if it did not exist, for read, edit and delete
    assert_eq!(
        app.get(&uri, Some(&stranger)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.patch(&uri, Some(&stranger), json!({ "title": "Hijacked" }))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.delete(&uri, Some(&stranger)).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, strangers_notes) = app.get("/api/v1/notes", Some(&stranger)).await;
    assert_eq!(strangers_notes["total"], 0);

    // and the owner's note is untouched
    assert_eq!(app.get(&uri, Some(&owner)).await.1["title"], "Mine");

    // no token at all
    assert_eq!(
        app.get("/api/v1/notes", None).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn input_is_validated_and_pinning_is_capped() {
    let app = spawn().await;
    let (_, token) = app.user("limits@example.com").await;

    let (status, body) = app
        .post("/api/v1/notes", Some(&token), json!({ "title": "" }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "validation_failed");
    assert!(body["error"]["details"]["title"].is_array());

    for number in 0..5 {
        let (status, _) = app
            .post(
                "/api/v1/notes",
                Some(&token),
                json!({ "title": format!("pinned {number}"), "pinned": true }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, body) = app
        .post(
            "/api/v1/notes",
            Some(&token),
            json!({ "title": "one too many", "pinned": true }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "bad_request");
}

#[tokio::test]
async fn listing_searches_paginates_and_puts_pinned_first() {
    let app = spawn().await;
    let (_, token) = app.user("lister@example.com").await;

    for (title, pinned) in [("alpha", false), ("beta", false), ("gamma", true)] {
        app.post(
            "/api/v1/notes",
            Some(&token),
            json!({ "title": title, "body": "shared words", "pinned": pinned }),
        )
        .await;
    }

    let (_, page) = app.get("/api/v1/notes?per_page=2", Some(&token)).await;
    assert_eq!(page["total"], 3);
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        page["items"][0]["title"], "gamma",
        "pinned notes come first"
    );

    let (_, page2) = app
        .get("/api/v1/notes?per_page=2&page=2", Some(&token))
        .await;
    assert_eq!(page2["items"].as_array().unwrap().len(), 1);

    let (_, found) = app.get("/api/v1/notes?q=BET", Some(&token)).await;
    assert_eq!(found["total"], 1);
    assert_eq!(found["items"][0]["title"], "beta");
}

#[tokio::test]
async fn a_read_only_api_key_can_read_notes_but_not_change_them() {
    let app = spawn().await;
    let (_, token) = app.user("keys@example.com").await;
    app.post("/api/v1/notes", Some(&token), json!({ "title": "Visible" }))
        .await;

    let (_, key) = app
        .post(
            "/api/v1/api-keys",
            Some(&token),
            json!({ "name": "reader", "scope": "read" }),
        )
        .await;
    let key = key["key"].as_str().unwrap();

    let (status, listed) = app
        .json_with(Method::GET, "/api/v1/notes", &[("x-api-key", key)], None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 1);

    // No extra code was needed for this: `AuthUser` refuses unsafe methods for read-only keys.
    let (status, _) = app
        .json_with(
            Method::POST,
            "/api/v1/notes",
            &[("x-api-key", key)],
            Some(json!({ "title": "Nope" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_pruning_job_deletes_only_old_unpinned_notes() {
    use api_starter_axum::infra::jobs::prune_old_notes;

    let app = spawn().await;
    let (owner_id, token) = app.user("pruned@example.com").await;

    for (title, pinned) in [
        ("stale", false),
        ("stale but pinned", true),
        ("fresh", false),
    ] {
        app.post(
            "/api/v1/notes",
            Some(&token),
            json!({ "title": title, "pinned": pinned }),
        )
        .await;
    }
    // age two of them by two years, straight in the database
    sqlx::query(
        "UPDATE notes SET updated_at = now() - interval '2 years'
         WHERE owner_id = $1::uuid AND title LIKE 'stale%'",
    )
    .bind(&owner_id)
    .execute(&app.state.db)
    .await
    .unwrap();

    let deleted = prune_old_notes::run(&app.state.db).await.unwrap();
    assert_eq!(deleted, 1);

    let (_, remaining) = app.get("/api/v1/notes", Some(&token)).await;
    let titles: Vec<&str> = remaining["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["stale but pinned", "fresh"]);
}
```

Run it:

```bash
cargo test --test notes
cargo fmt --all && cargo clippy --all-targets -- -D warnings
```

## 12. Try it for real

```bash
cargo run
TOKEN=$(curl -s -X POST localhost:3000/api/v1/auth/login -H 'content-type: application/json' \
  -d '{"email":"you@example.com","password":"..."}' | jq -r .access_token)

curl -X POST localhost:3000/api/v1/notes -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"title":"Groceries","body":"milk, eggs","pinned":true}'
curl "localhost:3000/api/v1/notes?q=milk" -H "authorization: Bearer $TOKEN"
```

Open http://localhost:3000/docs and the notes endpoints are there, with try-it-out.

## 13. Split by use case

The steps above keep each role in one file so the whole feature reads top to bottom. The
project's rule is that a role becomes a folder once it holds two or more use cases, which notes
does, so the finished feature looks like every other module:

```
src/modules/notes/
├── controllers/
│   ├── mod.rs            router(): the routes, pointing at the files below
│   ├── create_note.rs    POST   /notes
│   ├── list_notes.rs     GET    /notes
│   ├── get_note.rs       GET    /notes/{id}
│   ├── update_note.rs    PATCH  /notes/{id}
│   └── delete_note.rs    DELETE /notes/{id}
├── services/
│   ├── mod.rs            struct NotesService, new(), shared helpers (ensure_room_to_pin, the limit)
│   ├── create_note.rs    one `impl NotesService` block per use case
│   ├── list_notes.rs
│   ├── get_note.rs
│   ├── update_note.rs
│   └── delete_note.rs
├── repository.rs         one table, one concern: stays a single file
├── dto/  entity.rs  error.rs  mod.rs
```

Nothing about the types changes: there is still one `NotesService` and callers still write
`notes.create(...)`. Each file just adds methods to it:

```rust
// services/mod.rs: the struct, its constructor, and helpers several use cases share
mod create_note;
mod delete_note;
mod get_note;
mod list_notes;
mod update_note;

const MAX_PINNED_NOTES_PER_USER: i64 = 5;

#[derive(Clone)]
pub struct NotesService {
    repo: NotesRepository,
}

impl NotesService {
    pub fn new(repo: NotesRepository) -> Self { Self { repo } }

    async fn ensure_room_to_pin(&self, owner_id: Uuid) -> AppResult<()> { /* as in step 8 */ }
}
```

```rust
// services/create_note.rs: one use case
use super::NotesService;

impl NotesService {
    pub async fn create(&self, actor: &AuthUser, dto: CreateNoteDto) -> AppResult<NoteResponse> {
        /* as in step 8 */
    }
}
```

Private fields and helpers in `services/mod.rs` are visible to the files under it, because they
are its child modules. Controllers work the same way, with the router in `controllers/mod.rs`:

```rust
// controllers/mod.rs
pub mod create_note;
pub mod delete_note;
pub mod get_note;
pub mod list_notes;
pub mod update_note;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_notes::list).post(create_note::create))
        .route(
            "/{id}",
            get(get_note::get_one)
                .patch(update_note::update)
                .delete(delete_note::remove),
        )
}
```

In `infra/openapi.rs`, handlers are then listed by their file:
`crate::modules::notes::controllers::create_note::create`. Split a repository the same way once it
covers more than one table or concern (see `modules/oauth/repositories/`: states, identities,
grants).

## Done checklist

- [ ] Migration added, and it applies on an empty database and on top of the previous one
- [ ] SQL only in the repository; every query for user-owned data filters by owner
- [ ] Entity never returned directly; a DTO for every request and response
- [ ] Each error mapped to the right status in one `From<...> for AppError`
- [ ] Right guard on every handler (`AuthUser` / `SessionUser` / `AdminUser` / none)
- [ ] Handlers annotated with `#[utoipa::path]` and listed in `infra/openapi.rs`
- [ ] Tests cover success, validation failure, someone else's data, and no token
- [ ] `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` pass
- [ ] New settings added to `.env.example` and the README configuration table
- [ ] README's endpoint tables and [API reference](../README.md#api) updated

## Variations

| You want to... | Read |
|---|---|
| Restrict to admins, or share data between users | [endpoints.md: choosing a guard](endpoints.md#choosing-a-guard) |
| Rate limit a route | [endpoints.md: rate limiting](endpoints.md#rate-limiting-a-route) |
| Accept file uploads | [recipes.md: store files](recipes.md#store-files) |
| Send an email from your feature | [recipes.md: send an email](recipes.md#send-an-email) |
| Run work in the background or on a schedule | [recipes.md: background jobs](recipes.md#run-work-in-the-background) |
| Record who did what | [recipes.md: audit log](recipes.md#record-an-audit-log-entry) |

### A recurring job for the example

The notes feature also ships a daily job that deletes unpinned notes untouched for a year. It is
the complete pattern for scheduled work: a module with a `KIND`, a `run` function, and a
`reschedule_after`, registered in the worker. The mechanics are explained in
[recipes.md](recipes.md#run-work-in-the-background).

**src/infra/jobs/prune_old_notes.rs**

```rust
//! A recurring job: once a day, delete unpinned notes nobody has touched for a year.

use chrono::{Duration, Utc};
use sqlx::PgPool;

/// The name stored in `jobs.kind` and matched in `worker::dispatch`.
pub const KIND: &str = "prune_old_notes";
const INTERVAL: Duration = Duration::days(1);
const UNPINNED_NOTE_RETENTION: Duration = Duration::days(365);

/// Returns how many notes were deleted.
pub async fn run(db: &PgPool) -> Result<u64, sqlx::Error> {
    let cutoff = Utc::now() - UNPINNED_NOTE_RETENTION;
    let result = sqlx::query("DELETE FROM notes WHERE NOT pinned AND updated_at < $1")
        .bind(cutoff)
        .execute(db)
        .await?;
    Ok(result.rows_affected())
}

/// How long to wait before the next run.
pub fn reschedule_after() -> Duration {
    INTERVAL
}
```
