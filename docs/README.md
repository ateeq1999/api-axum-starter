# Documentation

Guides for building on this starter. The [project README](../README.md) is the reference for what
the API does today (endpoints, configuration, monitoring); these pages are about **changing it**:
adding features, endpoints and infrastructure.

## Start here

| If you want to... | Read |
|---|---|
| Understand how the code is organized and why | [architecture.md](architecture.md) |
| Add a complete feature (tables, endpoints, tests) | [adding-a-feature.md](adding-a-feature.md), a worked example with real, tested code |
| Add or change a single endpoint | [endpoints.md](endpoints.md) |
| Send email, run background jobs, store files, add a setting, audit, metrics | [recipes.md](recipes.md) |
| Write and run tests | [testing.md](testing.md) |
| Deploy to AWS on a small budget | [deploy.md](deploy.md) |
| Deploy the whole stack to the local floci emulator | [../infra/terraform/README.md](../infra/terraform/README.md) |

## The shortest path to a working feature

1. `git clone` the template and follow the README's *Quick start* (`cargo run -- setup`, `cargo run`).
2. Open [adding-a-feature.md](adding-a-feature.md) and copy the notes example, renaming it.
3. Change the migration, DTOs and rules; keep the layering.
4. `cargo test`, then try it at http://localhost:3000/docs.

## What is already built

You do not need to build these; use them. See the README for their endpoints.

| Capability | Where |
|---|---|
| Users, roles, admin management, soft delete | `modules/users` |
| Password, OAuth (Google, GitHub), passkeys, QR-code sign-in, TOTP two-factor | `modules/auth`, `oauth`, `passkeys`, `qr_login` |
| API keys with read/write scopes | `modules/api_keys` |
| Profile photos and general media uploads (disk or S3) | `modules/avatars`, `modules/media`, `infra/storage.rs` |
| Email templates, durable outbox, SMTP | `modules/mail` |
| Audit log of admin actions | `modules/audit_log` |
| Background job queue | `infra/jobs` |
| Prometheus metrics, Swagger UI, health checks | `infra/metrics.rs`, `infra/openapi.rs`, `modules/health` |
| Docker image, CI, Terraform for floci | `Dockerfile`, `.github/workflows`, `infra/terraform` |

## Conventions in one paragraph

Controllers call one service method and contain no SQL; services hold the rules and know nothing
about HTTP; repositories hold all the SQL; DTOs are the wire shapes and entities never leave the
server. Names are descriptive, errors are named after what went wrong, and every endpoint is
documented in Swagger and covered by an integration test. Details in
[architecture.md](architecture.md#conventions).
