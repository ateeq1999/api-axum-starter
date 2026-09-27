# Recipes

Short, self-contained how-tos for the things a feature usually needs beyond CRUD. Each assumes
you know the layers from [architecture.md](architecture.md).

- [Add a configuration setting](#add-a-configuration-setting)
- [Send an email](#send-an-email)
- [Run work in the background](#run-work-in-the-background)
- [Store files](#store-files)
- [Record an audit log entry](#record-an-audit-log-entry)
- [Count something in Prometheus](#count-something-in-prometheus)
- [Use another feature's service](#use-another-features-service)

## Add a configuration setting

Say notes should have a configurable pin limit, `MAX_PINNED_NOTES_PER_USER`.

1. **Read it** in `src/config/`. Small groups get their own struct in `features.rs`, built by
   `Config::from_env`. The helpers do the parsing:

   | Helper | Behavior |
   |---|---|
   | `optional("NAME", default)?` | Parses the value; **unset or blank uses the default**; a bad value is a startup error |
   | `required("NAME")?` | A startup error if unset or blank |
   | `non_empty("NAME")` | `Some(value)` only when set and not blank |

   ```rust
   // src/config/features.rs
   #[derive(Debug, Clone)]
   pub struct NotesConfig {
       pub max_pinned_notes_per_user: i64,
   }

   impl NotesConfig {
       pub(super) fn from_env() -> Result<Self, ConfigError> {
           Ok(Self {
               max_pinned_notes_per_user: optional("MAX_PINNED_NOTES_PER_USER", 5)?,
           })
       }
   }
   ```

   Add `pub notes: NotesConfig` to `Config` in `src/config/mod.rs`, build it in `from_env`, and
   re-export the type from `mod.rs`. Validate cross-field rules there (see `MediaConfig`, which
   refuses an upload limit larger than the body limit), so a bad combination fails at startup and
   not on the first request.

2. **Use it**: pass it into the service constructor in `src/state.rs`
   (`NotesService::new(repo, config.notes.clone())`) and store it on the service.
3. **Tests**: `tests/common/mod.rs::test_config()` builds a `Config` by hand, so add the new field
   there. A test that needs a different value calls `spawn_with(config)`.
4. **Document it**: an active line in `.env.example` (a blank value means "use the default") and a
   row in the README configuration table.

Secrets never go in the repository; only `.env.example` carries placeholders, and `.env` is
git-ignored.

## Send an email

Emails are templates plus a small struct. The service you call never touches SMTP: it asks
`MailService`, which renders, queues in a durable outbox, sends in the background, and retries.

1. **Templates** in `src/modules/mail/templates/`: `note_shared.html` extends the shared layout
   and uses the components; `note_shared.txt` is the plain-text version.

   ```html
   {% extends "layout.html" %}
   {% import "components.html" as ui %}

   {% block title %}A note was shared with you{% endblock %}
   {% block preview_text %}{{ sender }} shared a note with you.{% endblock %}
   {% block footer_reason %}You received this because someone shared a note with this address.{% endblock %}

   {% block content %}
   <h1 style="margin: 0 0 16px 0; font-size: 24px; line-height: 32px; font-weight: 700; color: #1C1917;">A note was shared with you</h1>
   <p style="margin: 0 0 16px 0; font-size: 16px; line-height: 26px; color: #1C1917;"><strong>{{ sender }}</strong> shared a note with you.</p>
   {{ ui::button(link, "Open the note") }}
   {{ ui::link_fallback(link) }}
   {% endblock %}
   ```

   The design is explained in the README's *Email design* section: table layout and inline
   styles only, because Gmail and Outlook ignore modern CSS. Values you pass in are HTML-escaped
   automatically.

2. **Message struct** in `src/modules/mail/messages/note_shared.rs`, modeled on
   `email_change_notice.rs`: a public struct with the variables, an askama `Html` and `Text`
   struct each, and a `render(&self, to)` that calls `super::assemble(to, "subject", text, html)`.
   Declare it with `pub mod note_shared;` in `messages/mod.rs`. (For the common "a link and an
   expiry" email, the `link_message!` macro there declares the whole thing in a few lines.)

3. **A send method** on `MailService`, in `modules/mail/service/senders.rs` next to the others:

   ```rust
   pub fn send_note_shared(&self, to: &str, sender: &str, link: &str) {
       self.dispatch(NoteShared { sender, link }.render(to));
   }
   ```

   Sending is fire-and-forget by design: a mail failure is logged (without the link) and never
   fails the request that triggered it. Use `self.link("/path", token)` to build a link to your
   frontend from `FRONTEND_URL`.

4. **Call it** from your service. Give the service a `MailService` in its constructor, the way
   `AuthService::new(db, users, mail, &config)` does.

5. **Test it**: the in-memory transport records what would be sent.

   ```rust
   let mails = app.mails_to("friend@example.com").await;
   assert_eq!(mails[0].subject, "A note was shared with you");
   assert!(mails[0].html.contains("Open the note"));
   ```

   Add the new message to `every_email_renders_a_complete_layout_and_text_part` in
   `modules/mail/service/tests.rs`, which checks every email for leftover template syntax, a title, the
   layout footer and a text part.

To preview it: `docker compose -f docker-compose.dev.yml up -d` starts Mailpit, then set
`MAIL_ENABLED=true`, `SMTP_HOST=127.0.0.1`, `SMTP_PORT=1025`, `SMTP_TLS=none` and read mail at
http://localhost:8025.

## Run work in the background

The job queue is a Postgres table claimed with `FOR UPDATE SKIP LOCKED`, running inside the same
process as the server: no extra service to deploy. Use it for anything that should not block a
request (the timeout is 10 seconds), should retry on failure, or should run on a schedule.

**Guarantees:** at-least-once. A failed job retries with backoff (1 s, 4 s, 16 s, then every 16 s)
and moves to a `dead` status after 5 attempts, kept for inspection. Up to 4 jobs run at once.
**Write handlers so running twice is harmless.**

### A recurring job

A recurring job runs, then enqueues itself for the next time, so it needs no cron. The notes
example ships one (`prune_old_notes`, shown in full at the end of
[adding-a-feature.md](adding-a-feature.md#a-recurring-job-for-the-example)). To add your own:

1. A module in `src/infra/jobs/` with a `KIND` constant, a `run` function and a
   `reschedule_after`, declared with `pub mod your_job;` in `jobs/mod.rs`.
2. Handle the kind in `dispatch` in `jobs/worker.rs` and re-enqueue:

   ```rust
   prune_old_notes::KIND => {
       let deleted = prune_old_notes::run(db).await.map_err(|e| e.to_string())?;
       tracing::info!(deleted, "pruned old notes");
       let repo = JobsRepository::new(db.clone());
       repo.enqueue(
           prune_old_notes::KIND,
           "{}",
           chrono::Utc::now() + prune_old_notes::reschedule_after(),
       )
       .await
       .map_err(|e| e.to_string())
   }
   ```

3. Schedule the first run at startup, next to the cleanup job in `Worker::run`.
   `ensure_scheduled` enqueues one only if none is pending or running, so restarts do not
   duplicate it:

   ```rust
   if let Err(error) = self.repo.ensure_scheduled(prune_old_notes::KIND).await {
       tracing::error!(%error, "could not schedule the initial note pruning job");
   }
   ```

4. Test `run` directly against a test database and assert on the rows it changed, as
   `tests/notes.rs` does. `tests/jobs.rs` shows how to test the queue mechanics.

### A one-off job

Enqueue with a JSON payload and read it back in the handler (`job.payload`). Do **not** re-enqueue:

```rust
JobsRepository::new(db.clone())
    .enqueue("send_weekly_digest", &serde_json::to_string(&payload)?, chrono::Utc::now())
    .await?;
```

`JobsRepository::new` only clones the pool, so create it wherever you need one.

## Store files

Go through `ObjectStorage` (`src/infra/storage.rs`), never straight to disk or S3. It is one handle
in `AppState` (`object_storage`) that is local disk by default and S3 when `S3_BUCKET` is set, so
your feature works in both without a change.

```rust
storage.put("notes/<id>/<file>.pdf", bytes, "application/pdf").await?;   // Vec<u8>
let bytes: Option<Vec<u8>> = storage.get("notes/<id>/<file>.pdf").await?; // None if missing
storage.delete("notes/<id>/<file>.pdf").await;                             // best effort
```

The rules that make it safe, all demonstrated in `modules/media/`:

- **Key = a fixed prefix for your feature + a server-generated id.** Never build a key from user
  input. `ObjectStorage::is_valid_key` rejects anything with `..`, empty segments or odd
  characters as a backstop.
- **Keep a database row for each file** (owner, type, size, original name). The bucket is storage;
  the row is what decides who may read it.
- **Detect the type from the bytes** (`infer`), never trust the client's `Content-Type`, and use
  an allow-list. Do not accept HTML or SVG uploads (they run script when served from your origin).
- **Serve through an authenticated endpoint** and keep the bucket private. Send `nosniff`, a
  sandboxing CSP, and `Content-Disposition: attachment` for anything that is not passive media.
- **Clean up:** delete the object if the database insert fails, and delete a user's files when
  the user is deleted.
- Files are buffered in memory (bounded by `MEDIA_MAX_UPLOAD_BYTES`), so this suits images,
  documents and short clips.

To let users upload media without a new feature, the `/api/v1/media` endpoints already exist.

## Record an audit log entry

For actions someone may later ask "who did that?" about, write an entry. It is durable, queryable,
and readable by admins at `GET /api/v1/audit-log`.

1. Add a name to the `action` module in `src/modules/audit_log/services/mod.rs` (constants, so a typo
   cannot create a new, undiscoverable action):

   ```rust
   pub const NOTE_EXPORTED: &str = "note.exported";
   ```

2. Give your service an `AuditLogService` (it is in `AppState` as `audit_log`; `UsersService` takes
   a clone in its constructor) and record after the action succeeds:

   ```rust
   self.audit_log
       .record(actor.id, action::NOTE_EXPORTED, Some(target_user_id), json!({ "note_id": id }))
       .await;
   ```

`record` never fails the request: a logging hiccup must not stop an admin from, say, deactivating a
compromised account. The `details` value is stored as JSON.

## Count something in Prometheus

Call the `metrics` macros at the point the event happens:

```rust
metrics::counter!("notes_created_total").increment(1);
metrics::counter!("note_export_total", "outcome" => "success").increment(1);
```

They appear on the metrics listener (`METRICS_BIND_ADDR`, default `127.0.0.1:9091`), separate from
the API. Keep label values to a small fixed set (an outcome, a provider), **never** an id or an
email, or the number of time series grows without bound.

## Use another feature's service

Pass the service into your constructor in `src/state.rs`, where the services are built in
dependency order, and store a clone (services are cheap to clone):

```rust
let notes = NotesService::new(NotesRepository::new(db.clone()), users.clone());
```

Depend on `users` (any feature may), not on a sibling feature that depends on you. If two features
would need each other, move the shared part into `common`, or define a trait in `common` and
implement it in one of them, as the API-key and session checks do.
