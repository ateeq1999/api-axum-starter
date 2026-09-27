# Deploying to AWS on a small budget

How this API runs in production on real AWS for **≈ $7.25 a month** (≈ $29 over 4 months, inside
a $30 budget), and what was verified. The operational kit itself (Terraform, server files,
scripts, and a 14-chapter manual) is kept **outside the repository**, because it contains state,
keys and secrets; this page is the public summary of it.

To try a similar shape locally for free, [infra/terraform/](../infra/terraform/README.md) deploys
the stack to the floci AWS emulator.

- [Result](#result)
- [Architecture](#architecture)
- [Cost](#cost)
- [What Terraform creates](#what-terraform-creates)
- [Configuration on the server](#configuration-on-the-server)
- [Security](#security)
- [Backups and restore](#backups-and-restore)
- [Email without a domain](#email-without-a-domain)
- [Deploying and updating](#deploying-and-updating)
- [Verification](#verification)
- [Lessons learned](#lessons-learned)
- [Limits](#limits)
- [Teardown](#teardown)

## Result

| | |
|---|---|
| Hosting | One AWS Lightsail instance, us-east-1 |
| URL | `https://<public-ip-with-dashes>.sslip.io` (no domain needed) |
| TLS | Let's Encrypt, obtained and renewed automatically by Caddy |
| Database | Postgres 16 in a container on the same server |
| Uploads | Private S3 bucket (`avatars/`, `media/`) |
| Email | Amazon SES (sandbox until a domain is added) |
| Backups | Nightly `pg_dump` to S3 (14 days) + daily Lightsail disk snapshot (7 days) |
| Spend guard | AWS Budgets, $7.50/month, email alerts |
| Provisioning | Terraform, run from the official Docker image (no local install) |

## Architecture

```
Internet ──HTTPS──► Lightsail instance: 1 GB RAM, 2 vCPU, 40 GB SSD, static public IPv4
                    firewall: 80 + 443 open; 22 from one admin IP only
                    ┌──────────────────── Docker Compose ─────────────────────┐
                    │ Caddy (TLS) ──► API container ──► Postgres 16 (volume)   │
                    └──────────────────────────────────────────────────────────┘
                      uploads + nightly dumps ─► S3 (private, encrypted, public access blocked)
                      email over SMTP :587 ────► SES
```

Why not the usual AWS pieces:

| Usual | Alone per month | Used instead |
|---|---|---|
| RDS Postgres | ≈ $12+ | Postgres in a container, backed up to S3 |
| Application Load Balancer | ≈ $16+ | Caddy on the server |
| EC2 + public IPv4 | ≈ $10.40 (`t4g.micro` + IPv4) | Lightsail, whose $7 price includes the IPv4 |
| Container registry | cents | `docker save \| ssh docker load` |
| Route 53 + domain | $0.50 + domain | sslip.io |

## Cost

us-east-1, checked against AWS on 2026-09-27.

| Item | 4 months |
|---|---|
| Lightsail `micro_3_0` (1 GB, 2 vCPU, 40 GB SSD, 2 TB transfer, IPv4 and static IP included) | $28.00 |
| Automatic snapshots (7 kept, incremental) | ≈ $0.60 |
| S3 storage and requests | ≈ $0.25 |
| SES ($0.10 per 1,000 emails) | ≈ $0.10 |
| IAM, AWS Budgets, sslip.io, Let's Encrypt | $0 |
| **Total** | **≈ $28.90** |

**Public IPv4 is not free.** Since February 2024 AWS bills every public IPv4 address, attached or
not, at about $3.65/month on EC2. Earlier versions of this page said an attached Elastic IP was
free; that is no longer true, and it is the main reason Lightsail beats EC2 at this budget.

Also changed: AWS accounts created after July 2025 get starting credits instead of the old
12-month free tier. This plan does not rely on either.

## What Terraform creates

15 resources, all tagged `Project = api-starter-prod`:

| Resource | Purpose |
|---|---|
| Lightsail instance (+ daily AutoSnapshot add-on) | The server |
| Lightsail static IP + attachment | A fixed IP, so the hostname and certificate never change |
| Lightsail public ports | The firewall: 80, 443, and 22 from one IP |
| Lightsail key pair (private key written locally) | SSH |
| S3 bucket + public access block + encryption + lifecycle | Uploads and backups; dumps expire after 14 days |
| IAM user + inline policy + access key | The server's only AWS credential (see [Security](#security)) |
| SES email identity | The verified sender |
| AWS Budget | $7.50/month; alerts at 80% actual and 100% forecast |

The static IP attachment and the firewall use `replace_triggered_by` on the instance: they
reference it by name, which a replacement keeps, so without it they would stay pointed at a
deleted server.

## Configuration on the server

The app reads a generated `.env` (mode 600). Notable values beyond the defaults in
`.env.example`:

| Variable | Value | Why |
|---|---|---|
| `BIND_ADDR` | `0.0.0.0:3000` | Caddy reaches the app over Docker's network |
| `TRUST_PROXY_HEADERS` | `true` | Exactly one trusted proxy (Caddy) in front, so rate limiting sees real client IPs |
| `CORS_ALLOWED_ORIGINS` | the deployed frontend (Vercel), `http://localhost:3001` for local frontend development, and the API host (Swagger UI) | Only these browser origins may call the API |
| `FRONTEND_URL` | the deployed frontend (Vercel) | Emailed links (`/reset-password`, `/verify-email`, `/confirm-email-change`), the OAuth hand-back (`/oauth/callback`) and the passkey relying party |
| `PUBLIC_API_URL` | the sslip.io URL | OAuth redirect URIs registered with Google and GitHub point at the API |
| `S3_BUCKET`, `AWS_*` | the bucket and the app user's key | Lightsail has no instance roles |
| `SMTP_*`, `MAIL_FROM` | SES SMTP endpoint, key-derived SMTP password | Email |
| `MAX_CONCURRENT_HASHES` | `4` | Each argon2 hash takes ~19 MB; bounded for 1 GB |
| `GOOGLE_*`, `GITHUB_*` | blank | Sign-in providers are off until configured |

Memory limits in Compose: Caddy 128 MB, API 400 MB, Postgres 320 MB (tuned: `shared_buffers=64MB`,
`max_connections=40`), plus a 1 GB swap file. Measured at idle: ≈ 130 MB for all three.

## Security

- **Firewall:** only 80 and 443 are public; SSH only from one IP; Postgres and the app port are
  reachable only inside the server.
- **One narrow credential on the server:** an IAM user limited to its bucket's `avatars/*` and
  `media/*` (read, write, delete), `backups/*` (write, read; it cannot delete backups), listing its
  own bucket, and `ses:SendRawEmail`. The same key yields the SES SMTP password. Rotating it is one
  Terraform `-replace` plus a redeploy.
- **The admin profile** that runs Terraform never reaches the server.
- **Bucket:** all four Block Public Access settings on, SSE-S3 encryption by default.
- **Updates:** Ubuntu's unattended security upgrades are enabled at first boot.
- **Secrets** (JWT secret, database and admin passwords, keys) are generated locally, kept only in
  the private kit, Terraform state and the server's `.env`; never in git.

## Backups and restore

| Layer | Schedule | Kept | Covers |
|---|---|---|---|
| `pg_dump --format=custom` to S3 `backups/` | 03:15 UTC, cron | 14 days (S3 lifecycle) | The database |
| Lightsail AutoSnapshot | 06:00 UTC | 7 days | The whole disk |

A restore script restores any dump either into a **scratch database** (to prove the backup works,
then dropped) or over the **live** database (API stopped meanwhile, confirmation required).
Worst-case data loss: up to one day.

## Email without a domain

With no domain, SES can only send **from one verified email address**, and a new account is in the
**SES sandbox**: mail reaches only verified addresses, at most 200 a day. That is enough to test
every flow (verification, reset, invitation, security notices) with your own inbox, not for real
users. Leaving the sandbox needs a domain (verified in SES with DKIM), a `no-reply@` sender on it,
and a production-access request. Sending "from Gmail" through SES also tends to land in spam for
anyone else.

## Deploying and updating

The server is too small to compile Rust, so the image is built locally with the repository's
`Dockerfile` and streamed over SSH (`docker save | gzip | ssh … docker load`). A deploy then copies
the Compose file, Caddyfile and `.env`, runs `docker compose up -d`, and waits for
`/health/ready`. Migrations run at API startup. Existing secrets are kept across deploys, so users
stay signed in. Downtime per deploy: a few seconds.

`.dockerignore` excludes `deploy/`, so the private kit (and its secrets) never enters a build
context.

## Verification

Checked on 2026-09-27 against the live deployment:

| Check | Result |
|---|---|
| TLS certificate | Let's Encrypt, issued to the sslip.io hostname, auto-renewing |
| `http://` | 308 redirect to `https://` |
| `/health/ready`, Swagger `/docs` | 200 |
| Bootstrap admin sign-in, `/users/me` | Works, role admin |
| Upload | Stored in S3 under `media/`; deleted cleanly |
| Real client IP behind Caddy | The caller's public IP, not Caddy's |
| Email | Password-reset email sent through SES (outbox drained, SES counter incremented) |
| Backup | Dump written to S3 |
| Restore | Scratch restore succeeded (all 16 migrations, users present) |
| Memory / disk | ≈ 130 MB used by the containers, 444 MB available; 12% disk |
| Budget | $7.50/month with both alerts |

## Lessons learned

- **Lightsail user data runs under `sh`.** Lightsail prepends its own `#!/bin/sh` setup and runs the
  combined script with `sh`, so a bash-only line (`set -o pipefail`) stopped the first boot before
  anything was installed. The bootstrap script is plain POSIX `sh`; `cloud-init status --long`
  shows such failures.
- **Watch the Docker build context.** A private folder inside the repo sent Terraform providers
  and a secrets file into the build context (140 MB+). Excluding it in `.dockerignore` brought it
  to under 1 MB.
- **Name-based references survive replacement.** Resources that point at an instance by name need
  `replace_triggered_by`, or they silently keep pointing at the deleted one.

## Limits

- One server: no failover; a reboot or failure is downtime.
- Database on the same disk as the app: protected by dumps and snapshots, up to a day of loss.
- 1 GB RAM: plenty for a starter, not for a launch spike (next step: the 2 GB plan at $12/month).
- Email is test-only until a domain is added.
- sslip.io is a free third-party service; a real domain removes that dependency.

## Teardown

Download what should be kept (`aws s3 sync` of the bucket, plus a final dump), allow the bucket's
deletion in Terraform, then `terraform destroy`. It removes all 15 resources, including the
server's snapshots. Afterwards, confirm no Lightsail instance, **static IP** or snapshot remains:
a detached static IP and manual snapshots are the usual leftover charges.
