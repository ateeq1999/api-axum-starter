# Deploying to floci with Terraform

Runs the whole stack on your machine against [floci](https://floci.io), the local AWS emulator
(the `infra-floci` container on `localhost:4566`): the API image in ECR, running on ECS, with an
RDS Postgres database, an S3 bucket for uploads, and secrets in Secrets Manager. It is the same
shape as a real AWS deployment, so the app's production configuration (S3 uploads, secrets from
the environment, `DATABASE_URL` to a managed Postgres) gets exercised before it ships.

For a real AWS deployment on a small budget, see [deploy.md](../../deploy.md).

## Quick start

Needs Docker and floci running. Terraform itself is optional: `tf.sh` uses your installed
`terraform`, or falls back to the official `hashicorp/terraform` image.

```bash
cd infra/terraform
./deploy.sh
```

That builds the project's Dockerfile (the first Rust build takes several minutes), pushes the
image to ECR, creates everything else, and waits until the API answers. Then:

| What | Where |
|---|---|
| API | http://localhost:3000 |
| Swagger UI | http://localhost:3000/docs |
| Admin login | printed when `deploy.sh` finishes (see [The administrator account](#the-administrator-account)) |
| Mail sent by the API | http://localhost:8025 (Mailpit) |
| Logs | `docker logs $(docker ps -q --filter name=floci-ecs) --tail 50` |

Tear it down with `./tf.sh destroy`.

```bash
SKIP_BUILD=1 ./deploy.sh      # reuse the local api-starter-axum:local image
IMAGE_TAG=v2 ./deploy.sh      # push and run another tag
API_HOST_PORT=8080 ./deploy.sh
```

### The administrator account

`deploy.sh` prints the admin email and password when it finishes. Choose your own, or let it
generate one:

```bash
./deploy.sh                                        # generated password, printed at the end
ADMIN_PASSWORD='my-long-passphrase' ./deploy.sh    # your own (8-128 characters)
ADMIN_EMAIL=me@example.com ./deploy.sh             # a different admin email
./tf.sh output -raw admin_password                 # show the password again later
```

The values reach Terraform through the environment, never as a command-line argument, so the
password does not appear in the process list. The API creates the admin only on first start, when
the database has no administrator yet. If you redeploy over an existing database with a different
password, the script warns you (it tries one login), because the old account keeps its old
password: change it in the app (`POST /api/v1/auth/password/change`), or `./tf.sh destroy` first
to start from an empty database.

## What it creates

| File | Resources |
|---|---|
| `registry.tf` | ECR repository `api-starter-axum` |
| `database.tf` | RDS Postgres 16 (floci runs a real Postgres container) and its subnet group |
| `storage.tf` | Private S3 bucket `api-starter-uploads` for `avatars/` and `media/` |
| `secrets.tf` | Generated JWT secret, database password and admin password, stored in Secrets Manager |
| `iam.tf` | Task execution role (pull image, write logs, read the three secrets) and task role (only `avatars/*` and `media/*` in the bucket) |
| `ecs.tf` | Log group, cluster, task definition and service running the API |
| `network.tf` | Uses floci's default VPC; a security group for the database |

The API container gets its configuration exactly like production would provide it: plain
settings as environment variables, `JWT_SECRET`, `DATABASE_URL` and `ADMIN_PASSWORD` injected by
ECS from Secrets Manager. Your `.env` is not used and not touched. floci also injects
`AWS_ENDPOINT_URL`, test credentials and the region into task containers, which is all the S3
uploads need (`S3_BUCKET` is set by Terraform).

## Variables

Override with `-var name=value` (via `./tf.sh apply -var ...`) or a `terraform.tfvars` file
(git-ignored).

| Variable | Default | Purpose |
|---|---|---|
| `frontend_url` | `http://localhost:3001` | Links in emails and the default CORS origin |
| `cors_allowed_origins` | blank (= `frontend_url`) | Comma-separated origins |
| `admin_email` | `admin@example.com` | Bootstrap administrator. Blank creates none |
| `admin_password` | blank (= generated) | Its password, 8-128 characters. Prefer `ADMIN_PASSWORD=... ./deploy.sh` |
| `require_verified_email` | `false` | Block login until the email is verified |
| `smtp_host` / `smtp_port` | `infra-mailpit` / `1025` | Mail server. Blank host turns sending off |
| `api_host_port` | `3000` | Port the API is published on |
| `container_image_tag` | `latest` | Image tag the service runs |
| `database_instance_class`, `database_allocated_storage_gb` | `db.t3.micro`, `5` | RDS sizing |
| `task_cpu_units`, `task_memory_mib` | `256`, `512` | Container size |

## floci behaviors worth knowing

- **The task runs in bridge mode on the EC2 launch type**, not Fargate. floci publishes a task's
  port on your machine only for bridge networking with an explicit `hostPort`; awsvpc tasks are
  reachable only from inside the Docker network.
- **The task shares floci's Docker network (`infra`)**, so it reaches RDS, floci, Mailpit and
  Postgres by name or IP. That is why the default `smtp_host` is the Mailpit container.
- **Creating RDS takes about 80 seconds** (it starts a real Postgres container).
- **State is local** (`terraform.tfstate`, git-ignored). If floci is wiped, run
  `rm -rf terraform.tfstate* .terraform` and `./deploy.sh` again; if only Terraform's state is
  lost, `deploy.sh` re-imports the ECR repository automatically (floci can answer
  `CreateRepository` slowly enough that the provider retries a create that already succeeded).
- **Endpoints are per service.** `providers.tf` overrides the endpoint for each service the stack
  uses. Adding a resource for another service (SQS, SES, ...) needs its endpoint added there, or
  the call goes to real AWS.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `floci is not answering on http://localhost:4566` | Start the floci container. |
| API never becomes ready | `docker logs` of the `floci-ecs-...` container. A wrong bucket or database shows up immediately: the API refuses to start. |
| `InvalidClientTokenId` from a service | That service has no endpoint override in `providers.tf`, so Terraform reached real AWS. |
| Port 3000 already in use | `API_HOST_PORT=8080 ./deploy.sh`. |
| `docker build` fails at `utoipa-swagger-ui` | Fixed: the crate's `vendored` feature embeds Swagger UI, so the build needs no `curl` and no network access for it. |

## Real AWS

Only `providers.tf` is floci-specific (endpoints, fixed credentials, skipped account checks).
The resources are ordinary AWS ones, but a real deployment would also want a load balancer and
private networking, and on this project's small-budget path it uses a single EC2 instance
instead of ECS: see [deploy.md](../../deploy.md).
