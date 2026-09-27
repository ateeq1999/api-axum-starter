#!/usr/bin/env bash
# Deploys the API to the local floci emulator end to end:
#
#   1. create the ECR repository (and nothing else yet),
#   2. build the project's Dockerfile and push the image to it,
#   3. create everything else: RDS Postgres, S3 bucket, secrets, IAM, ECS service,
#   4. wait until the API answers.
#
# The order matters: the ECS service starts a container the moment it exists, so the image has
# to be in the registry first.
#
#   ./deploy.sh                    build, push and deploy
#   SKIP_BUILD=1 ./deploy.sh       reuse the local api-starter-axum:local image
#   IMAGE_TAG=v2 ./deploy.sh       push and run a different tag
#
# Administrator account (created on first start only, when the database has no admin yet):
#   ./deploy.sh                                   generated password, printed when it finishes
#   ADMIN_PASSWORD='my-long-passphrase' ./deploy.sh   your own password (8-128 characters)
#   ADMIN_EMAIL=me@example.com ./deploy.sh        a different admin email
#
# Tear everything down with:  ./tf.sh destroy
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

# Handed to Terraform through the environment (never as -var), so the password does not show up
# in the process list. tf.sh forwards these into its Docker fallback.
[ -n "${ADMIN_PASSWORD:-}" ] && export TF_VAR_admin_password="$ADMIN_PASSWORD"
[ -n "${ADMIN_EMAIL:-}" ] && export TF_VAR_admin_email="$ADMIN_EMAIL"

image_tag="${IMAGE_TAG:-latest}"
local_image="api-starter-axum:local"
api_port="${API_HOST_PORT:-3000}"

echo "==> Checking floci is up"
if ! curl -fsS -m 5 http://localhost:4566/_floci/health >/dev/null; then
  echo "floci is not answering on http://localhost:4566. Start it first." >&2
  exit 1
fi

echo "==> terraform init"
./tf.sh init -input=false >/dev/null

echo "==> Creating the ECR repository"
repository_name="${PROJECT_NAME:-api-starter}-axum"
ecr_log="$(mktemp)"
if ! ./tf.sh apply -input=false -auto-approve -no-color -target=aws_ecr_repository.api >"$ecr_log" 2>&1; then
  # floci can be slow to answer CreateRepository, and the provider then retries a create that
  # already succeeded ("RepositoryAlreadyExistsException"). Adopt the repository and go on.
  if grep -q "RepositoryAlreadyExistsException" "$ecr_log"; then
    echo "    repository already exists in floci, importing it into the state"
    ./tf.sh import -input=false -no-color aws_ecr_repository.api "$repository_name" >/dev/null
  else
    cat "$ecr_log" >&2
    exit 1
  fi
fi
repository_url="$(./tf.sh output -raw ecr_repository_url | tr -d '\r')"
echo "    $repository_url"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  echo "==> Building the image (the first Rust build takes several minutes)"
  docker build -t "$local_image" ../..
fi

echo "==> Pushing $repository_url:$image_tag"
docker tag "$local_image" "$repository_url:$image_tag"
docker push "$repository_url:$image_tag"

echo "==> Creating the rest of the stack (RDS takes about a minute)"
./tf.sh apply -input=false -auto-approve -var "container_image_tag=$image_tag" -var "api_host_port=$api_port"

echo "==> Waiting for the API on http://localhost:$api_port"
for _ in $(seq 1 60); do
  if curl -fsS -m 3 "http://localhost:$api_port/health/ready" >/dev/null 2>&1; then
    admin_email="$(./tf.sh output -raw admin_email | tr -d '\r')"
    admin_password="$(./tf.sh output -raw admin_password | tr -d '\r')"

    echo
    echo "API is up:      http://localhost:$api_port"
    echo "Swagger UI:     http://localhost:$api_port/docs"
    echo "Mail (Mailpit): http://localhost:8025"
    echo
    echo "Admin email:    $admin_email"
    if [ -n "$admin_email" ]; then
      if [ -n "${ADMIN_PASSWORD:-}" ]; then
        echo "Admin password: the one you set in ADMIN_PASSWORD"
      else
        echo "Admin password: $admin_password"
        echo "                (generated; show it again with ./tf.sh output -raw admin_password)"
      fi

      # The API only creates the admin when none exists yet, so on a redeploy over an existing
      # database the password above may not be the real one. One login attempt tells us.
      login_status="$(curl -s -o /dev/null -w '%{http_code}' -m 10 \
        -X POST "http://localhost:$api_port/api/v1/auth/login" \
        -H 'content-type: application/json' \
        -d "{\"email\":\"$admin_email\",\"password\":\"$admin_password\"}" || true)"
      if [ "$login_status" != "200" ]; then
        echo
        echo "WARNING: signing in with these credentials returned HTTP $login_status." >&2
        echo "An administrator already exists in this database, so ADMIN_EMAIL/ADMIN_PASSWORD were" >&2
        echo "ignored. Use that account's existing password, change it in the app, or wipe the" >&2
        echo "database with ./tf.sh destroy and deploy again." >&2
      fi
    fi
    exit 0
  fi
  sleep 3
done

echo "The API did not become ready in time. Look at its logs with:" >&2
echo "  docker logs \$(docker ps -q --filter name=floci-ecs) --tail 50" >&2
exit 1
