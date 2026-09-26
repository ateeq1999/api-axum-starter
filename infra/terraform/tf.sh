#!/usr/bin/env bash
# Runs Terraform against the local floci emulator. Use it exactly like `terraform`:
#
#   ./tf.sh init
#   ./tf.sh plan
#   ./tf.sh apply
#   ./tf.sh output -raw admin_password
#   ./tf.sh destroy
#
# With Terraform installed, it runs natively and talks to floci on localhost:4566. Without it, it
# runs the official hashicorp/terraform image on floci's Docker network (`infra` by default),
# where floci is reachable as http://floci:4566. Override with FLOCI_DOCKER_NETWORK.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

if command -v terraform >/dev/null 2>&1; then
  export TF_VAR_floci_endpoint="${TF_VAR_floci_endpoint:-http://localhost:4566}"
  exec terraform "$@"
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "Neither terraform nor docker is installed. Install one of them." >&2
  exit 1
fi

docker_network="${FLOCI_DOCKER_NETWORK:-infra}"
# Git Bash on Windows rewrites /work-style paths and gives the mount a POSIX path Docker
# Desktop cannot use, so disable that and pass the native Windows path.
export MSYS_NO_PATHCONV=1
project_directory="$(pwd -W 2>/dev/null || pwd)"

interactive_flags=()
[ -t 0 ] && [ -t 1 ] && interactive_flags=(-it)

exec docker run --rm "${interactive_flags[@]}" \
  --network "$docker_network" \
  -e TF_VAR_floci_endpoint="${TF_VAR_floci_endpoint:-http://floci:4566}" \
  -e TF_IN_AUTOMATION=1 \
  -v "$project_directory:/work" \
  -w /work \
  hashicorp/terraform:latest "$@"
