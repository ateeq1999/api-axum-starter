# Deploying to AWS on a $30 / 6-month budget

A from-scratch manual for running this API on AWS for about six months on roughly $30 of
infrastructure spend. It is written for a single low-traffic deployment (a personal project,
demo, or small starter), not a highly-available production fleet — see [Known limits and
tradeoffs](#known-limits-and-tradeoffs) for what you are deliberately giving up.

- [Assumptions](#assumptions)
- [Budget](#budget)
- [Architecture](#architecture)
- [1. Local prerequisites](#1-local-prerequisites)
- [2. AWS account and IAM user](#2-aws-account-and-iam-user)
- [3. Security group](#3-security-group)
- [4. SSH key pair](#4-ssh-key-pair)
- [5. Launch the EC2 instance](#5-launch-the-ec2-instance)
- [6. Elastic IP](#6-elastic-ip)
- [7. DNS](#7-dns)
- [8. Server setup: Docker](#8-server-setup-docker)
- [9. Build and publish the image](#9-build-and-publish-the-image)
- [10. Production `.env`](#10-production-env)
- [11. `docker-compose.prod.yml` and Caddy](#11-docker-composeprodyml-and-caddy)
- [12. First boot and verification](#12-first-boot-and-verification)
- [13. Backups](#13-backups)
- [14. Cost controls](#14-cost-controls)
- [15. Redeploying updates](#15-redeploying-updates)
- [16. Security hardening checklist](#16-security-hardening-checklist)
- [17. Monitoring within the RAM budget](#17-monitoring-within-the-ram-budget)
- [Known limits and tradeoffs](#known-limits-and-tradeoffs)
- [Teardown after 6 months](#teardown-after-6-months)

## Assumptions

This manual makes the following assumptions explicit, since they are what makes $30/6 months
achievable at all. If any of them don't hold for you, the budget math changes — the
[Budget](#budget) section shows both a free-tier and a paid-from-day-one scenario so you can
tell which applies.

1. **Region is `us-east-1`** (N. Virginia). It is consistently AWS's cheapest and most
   feature-complete region. Prices are a few percent to noticeably higher elsewhere.
2. **Single instance, single AZ, no load balancer, no NAT gateway, no RDS Multi-AZ.** An
   Application Load Balancer alone costs more than this entire 6-month budget; a NAT gateway
   costs about $32/month by itself. Both are skipped: the one EC2 instance sits in a public
   subnet of the default VPC with a locked-down security group instead.
3. **Traffic is low** — a personal project, demo, or small starter, not a product with real
   users yet. This keeps you comfortably inside AWS's always-free 100 GB/month outbound data
   transfer allowance and inside a `t3.nano`/`t3.micro`-class CPU budget. The app's own
   `RATE_LIMIT_PER_MINUTE` and `MAX_CONCURRENT_HASHES` settings put a ceiling on load, but they
   don't cap your AWS bill if someone deliberately hammers the public endpoints — that risk is
   accepted, not engineered away, at this budget.
4. **You are, or might be, inside a new AWS account's 12-month Free Tier window.** This is
   treated as the common case for someone budgeting this tightly, because it changes the
   architecture: free tier gives you 750 hrs/month of `t3.micro` (or `t2.micro`) EC2 *and*
   750 hrs/month of a `db.t3.micro`/`db.t4g.micro` RDS Postgres instance, each for 12 months.
   That lets the app and the database run as two separate, free, always-on resources. If your
   account is older than 12 months or has already used its free tier, none of that compute is
   free — [Budget](#budget) gives a paid fallback architecture (self-hosted Postgres in a
   container alongside the app, on a single smaller instance) that still fits $30/6 months.
5. **You already have, or are willing to register, one domain name** you can point a DNS
   record at. TLS via Let's Encrypt (used here through Caddy) needs a real domain — it cannot
   issue a certificate for a bare IP address. **Domain registration is not included in the $30
   budget** (roughly $10–15/year on top, for a `.com`); if you already own a domain, adding one
   subdomain (`api.yourdomain.com`) costs nothing extra.
6. **The $30 figure covers infrastructure only**: compute, storage, the Elastic IP, DNS, and
   backups. It excludes domain registration (point 5) and excludes outbound email if you turn
   `MAIL_ENABLED=true` and send non-trivial volume through SES (small volumes are a few cents;
   check current SES pricing before relying on any specific free allowance, since AWS has
   changed SES's free tier terms before).
7. **You accept the operational tradeoffs of one self-managed instance**: manual OS patching,
   no auto-scaling, no automatic failover, and — in the paid/no-free-tier scenario —
   self-hosted Postgres instead of RDS, meaning you are responsible for your own backups
   ([13. Backups](#13-backups) covers this).
8. **This is a personal or small-team AWS account** with no organizational SCPs, mandatory
   tagging, or centralized logging to work around. If your AWS account is managed by an
   organization, some steps (IAM user creation, budget alerts) may need an administrator.
9. **You're comfortable with basic SSH and Docker.** Every command needed is given verbatim,
   but the manual does not re-teach what SSH, Docker, or a security group are.
10. **Prices drift.** The figures below are approximate `us-east-1` on-demand prices at the
    time of writing. AWS changes prices; verify current numbers with the [AWS Pricing
    Calculator](https://calculator.aws) before committing, especially if your 6-month window is
    far in the future.

## Budget

### Scenario A — new/free-tier-eligible account (recommended if it applies to you)

EC2 and RDS compute is free for 12 months, so the entire 6-month budget goes to the handful of
things free tier doesn't cover.

| Item | Monthly | 6 months |
| --- | --- | --- |
| EC2 `t3.micro` (app), free tier (750 hrs/mo) | $0.00 | $0.00 |
| RDS `db.t3.micro` Postgres, single-AZ, free tier (750 hrs/mo, 20 GB gp2) | $0.00 | $0.00 |
| EBS root volume, free tier (30 GB) | $0.00 | $0.00 |
| Elastic IP, attached to a running instance | $0.00 | $0.00 |
| S3 backup bucket (pg_dump copies, belt-and-suspenders on top of RDS's own backups), free tier (5 GB) | $0.00 | $0.00 |
| Route 53 hosted zone (only if you don't already have DNS elsewhere) | $0.50 | $3.00 |
| Data transfer out, free tier (100 GB/mo) | $0.00 | $0.00 |
| **Total** | | **≈ $3.00** (or $0 if you skip Route 53) |

This leaves roughly $27 of headroom for price drift, a traffic spike, or running a month past
6 months. It is also the architecturally cleaner option: RDS handles automated backups and
patching for you, so [11.](#11-docker-composeprodyml-and-caddy) only needs to run the app and
Caddy, not Postgres, in containers.

### Scenario B — existing account, on-demand pricing throughout

No RDS (a `db.t3.micro` alone runs ≈$12/month on-demand — over budget by itself), and a smaller
instance to keep compute under control. Postgres runs self-hosted in a container next to the
app, on the same instance.

| Item | Monthly | 6 months |
| --- | --- | --- |
| EC2 `t3.nano` (0.5 GiB RAM), on-demand | ≈ $3.80 | ≈ $22.80 |
| EBS gp3 volume, 10 GB | ≈ $0.80 | ≈ $4.80 |
| Elastic IP, attached to a running instance | $0.00 | $0.00 |
| S3 backup bucket (pg_dump exports), a few hundred MB | ≈ $0.02 | ≈ $0.12 |
| Route 53 hosted zone (optional) | $0.50 | $3.00 |
| Data transfer out, within the always-free 100 GB/mo tier | $0.00 | $0.00 |
| **Total** | | **≈ $27.72–$30.72** |

This is tight — it assumes you skip Route 53 (use an existing DNS provider instead) if you want
real margin, and it assumes traffic stays light enough to avoid data transfer charges. A
`t3.nano` has only 0.5 GiB of RAM, which is enough for the app, Postgres, and Caddy together
only with a swap file as a safety margin — [8.](#8-server-setup-docker) sets one up.

If Scenario B's margin is too thin for your comfort, the honest fix is to raise the budget or
shorten the window, not to add resources this manual then has to caveat away — a `t3.micro`
(1 GiB, more comfortable) on-demand for 6 months alone runs ≈$45.5, which does not fit $30.

## Architecture

```
                          Internet
                             |
                    Elastic IP (public)
                             |
                    security group: 22 (your IP only),
                    80/tcp, 443/tcp (world)
                             |
        +--------------------------------------------+
        |  EC2 instance (t3.nano or t3.micro)         |
        |                                              |
        |   +--------+     +-----+     +-------------+ |
        |   | Caddy  |---->| API |---->| Postgres    | |   Scenario B only —
        |   | :80/443|     |:3000|     | (container) | |   Scenario A uses
        |   +--------+     +-----+     +-------------+ |   RDS instead
        |        (Docker Compose, one bridge network)  |
        +--------------------------------------------+
                             |
                cron: pg_dump -> S3 bucket (lifecycle: expire after 14 days)
```

Caddy terminates TLS (automatic Let's Encrypt certificates, renewed automatically) and reverse
proxies to the app container over the Docker network — the app itself is never exposed
directly to the internet. `/metrics` (port 9091) is not published anywhere; there's no budget
headroom for a self-hosted Prometheus/Grafana stack on a 0.5–1 GiB instance, so monitoring
relies on the app's own `/health/live` and `/health/ready` endpoints plus (optionally) a free
external uptime checker — see [17.](#17-monitoring-within-the-ram-budget).

## 1. Local prerequisites

On your own machine (not the EC2 instance):

- [AWS CLI v2](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html),
  configured (`aws configure`) with credentials for the IAM user from step 2.
- Docker, to build and push the release image. **Do not build the image on the EC2 instance** —
  `cargo build --release` on this project needs well over 1 GiB of RAM and several minutes of
  CPU; a `t3.nano`/`t3.micro` will swap-thrash or OOM. Build locally (or in CI) and push a
  finished image instead.
- An SSH client (built into macOS/Linux/modern Windows).

## 2. AWS account and IAM user

Using the root account's long-lived credentials day-to-day is a standing risk, not a one-time
convenience — create an IAM user for this deployment instead:

```bash
aws iam create-user --user-name api-starter-deployer

# A single custom policy covering everything this manual does. Precise least-privilege
# resource-level scoping is hard here because several ARNs (the instance, the Elastic IP,
# the bucket) don't exist until you create them in later steps — this is deliberately a
# capability-scoped, not resource-scoped, policy. Tighten it further once resources exist
# if you want to; that refinement is out of scope for a budget-focused manual.
cat > deployer-policy.json <<'EOF'
{
  "Version": "2012-10-17",
  "Statement": [
    { "Effect": "Allow", "Action": ["ec2:*"], "Resource": "*" },
    { "Effect": "Allow", "Action": ["s3:*"], "Resource": "*" },
    { "Effect": "Allow", "Action": ["rds:*"], "Resource": "*" },
    { "Effect": "Allow", "Action": ["route53:*"], "Resource": "*" },
    { "Effect": "Allow", "Action": ["budgets:*", "ce:GetCostAndUsage"], "Resource": "*" },
    { "Effect": "Allow", "Action": ["ses:SendEmail", "ses:SendRawEmail", "ses:VerifyDomainIdentity", "ses:VerifyEmailIdentity"], "Resource": "*" },
    { "Effect": "Allow", "Action": ["ssm:GetParameters", "ssm:GetParameter"], "Resource": "*" },
    { "Effect": "Allow", "Action": ["iam:CreateRole", "iam:GetRole", "iam:PutRolePolicy", "iam:CreateInstanceProfile", "iam:GetInstanceProfile", "iam:AddRoleToInstanceProfile", "iam:PassRole"], "Resource": "*" }
  ]
}
EOF
aws iam put-user-policy --user-name api-starter-deployer \
  --policy-name api-starter-deploy \
  --policy-document file://deployer-policy.json

aws iam create-access-key --user-name api-starter-deployer
# Save the AccessKeyId/SecretAccessKey it prints, then:
aws configure --profile api-starter
# Use that profile for every command below: add `--profile api-starter`, or
# `export AWS_PROFILE=api-starter` for the rest of this session.
```

Drop the `rds`/`route53`/`ses` statements if you know you're on Scenario B and skipping RDS,
Route 53, and mail.

## 3. Security group

```bash
VPC_ID=$(aws ec2 describe-vpcs --filters Name=isDefault,Values=true --query 'Vpcs[0].VpcId' --output text)

SG_ID=$(aws ec2 create-security-group \
  --group-name api-starter-sg \
  --description "api-starter-axum: SSH from me, HTTP/HTTPS from anywhere" \
  --vpc-id "$VPC_ID" \
  --query GroupId --output text)

MY_IP=$(curl -s https://checkip.amazonaws.com)/32

aws ec2 authorize-security-group-ingress --group-id "$SG_ID" \
  --protocol tcp --port 22 --cidr "$MY_IP"
aws ec2 authorize-security-group-ingress --group-id "$SG_ID" \
  --protocol tcp --port 80 --cidr 0.0.0.0/0
aws ec2 authorize-security-group-ingress --group-id "$SG_ID" \
  --protocol tcp --port 443 --cidr 0.0.0.0/0
```

Deliberately not opened: 3000 (app) and 9091 (metrics) — both stay reachable only from inside
the instance/Docker network, never from the public internet. If your home/office IP changes,
re-run the `authorize-security-group-ingress` line for port 22 with the new `$MY_IP` (and
`revoke-security-group-ingress` the old one).

## 4. SSH key pair

```bash
aws ec2 create-key-pair --key-name api-starter-key \
  --query 'KeyMaterial' --output text > api-starter-key.pem
chmod 400 api-starter-key.pem
```

Keep this file out of the git repo — it is a credential, not project source.

## 5. Launch the EC2 instance

Look up the current Ubuntu 24.04 LTS (x86_64) AMI for `us-east-1` rather than hardcoding an ID,
since Canonical publishes new AMI IDs with every patch release:

```bash
AMI_ID=$(aws ssm get-parameters \
  --names /aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id \
  --query 'Parameters[0].Value' --output text)

SUBNET_ID=$(aws ec2 describe-subnets \
  --filters Name=vpc-id,Values="$VPC_ID" Name=default-for-az,Values=true \
  --query 'Subnets[0].SubnetId' --output text)

# Scenario A (free tier): --instance-type t3.micro
# Scenario B (paid):      --instance-type t3.nano
INSTANCE_ID=$(aws ec2 run-instances \
  --image-id "$AMI_ID" \
  --instance-type t3.micro \
  --key-name api-starter-key \
  --security-group-ids "$SG_ID" \
  --subnet-id "$SUBNET_ID" \
  --associate-public-ip-address \
  --block-device-mappings '[{"DeviceName":"/dev/sda1","Ebs":{"VolumeSize":10,"VolumeType":"gp3","DeleteOnTermination":true}}]' \
  --tag-specifications 'ResourceType=instance,Tags=[{Key=Name,Value=api-starter-axum}]' \
  --query 'Instances[0].InstanceId' --output text)

aws ec2 wait instance-running --instance-ids "$INSTANCE_ID"
```

x86_64 (`t3`/`t3a`), not Graviton (`t4g`), is used throughout this manual on purpose: the
project's `Dockerfile` produces whatever architecture you build it on, and building on an
ordinary x86_64 laptop/CI runner gives you an x86_64 image with zero cross-compilation setup.
`t4g` instances are slightly cheaper, but only worth the extra step (`docker buildx build
--platform linux/arm64`, which is slower without native arm64 hardware) if you're chasing every
last dollar.

Scenario A only — Postgres via RDS free tier:

```bash
DB_SUBNET_A=$(aws ec2 describe-subnets --filters Name=vpc-id,Values="$VPC_ID" --query 'Subnets[0].SubnetId' --output text)
DB_SUBNET_B=$(aws ec2 describe-subnets --filters Name=vpc-id,Values="$VPC_ID" --query 'Subnets[1].SubnetId' --output text)
aws rds create-db-subnet-group --db-subnet-group-name api-starter-db-subnets \
  --db-subnet-group-description "api-starter-axum" \
  --subnet-ids "$DB_SUBNET_A" "$DB_SUBNET_B"

DB_SG_ID=$(aws ec2 create-security-group --group-name api-starter-db-sg \
  --description "Postgres, app instance only" --vpc-id "$VPC_ID" --query GroupId --output text)
aws ec2 authorize-security-group-ingress --group-id "$DB_SG_ID" \
  --protocol tcp --port 5432 --source-group "$SG_ID"

DB_PASSWORD=$(openssl rand -base64 24 | tr -d '/+=')
aws rds create-db-instance \
  --db-instance-identifier api-starter-db \
  --db-instance-class db.t3.micro \
  --engine postgres \
  --engine-version 16 \
  --master-username postgres \
  --master-user-password "$DB_PASSWORD" \
  --allocated-storage 20 \
  --db-subnet-group-name api-starter-db-subnets \
  --vpc-security-group-ids "$DB_SG_ID" \
  --backup-retention-period 7 \
  --no-multi-az \
  --no-publicly-accessible \
  --db-name api_starter_db

aws rds wait db-instance-available --db-instance-identifier api-starter-db
aws rds describe-db-instances --db-instance-identifier api-starter-db \
  --query 'DBInstances[0].Endpoint.Address' --output text
# Save this endpoint and $DB_PASSWORD — they go into DATABASE_URL in step 10.
```

`--no-multi-az` and `--backup-retention-period 7` are deliberate: Multi-AZ doubles the RDS cost
(and is outside free tier); a 7-day automated backup window is included free and is your actual
disaster-recovery story in Scenario A, making the manual S3 backup in [13.](#13-backups)
belt-and-suspenders rather than load-bearing.

## 6. Elastic IP

```bash
ALLOC_ID=$(aws ec2 allocate-address --query AllocationId --output text)
aws ec2 associate-address --instance-id "$INSTANCE_ID" --allocation-id "$ALLOC_ID"
PUBLIC_IP=$(aws ec2 describe-addresses --allocation-ids "$ALLOC_ID" --query 'Addresses[0].PublicIp' --output text)
echo "$PUBLIC_IP"
```

An Elastic IP is free only while it's associated with a running instance. If you ever stop the
instance for more than a few minutes, either terminate the Elastic IP too or accept it will
start accruing a small idle charge (≈$3.60/month) — this is one of the few AWS charges that
bills you for doing *less*, not more.

## 7. DNS

Point `api.yourdomain.com` (or whatever subdomain you choose) at `$PUBLIC_IP` with an `A`
record, using whichever DNS provider already hosts your domain. If you have no existing DNS
provider and want everything inside AWS, create a Route 53 hosted zone instead
(this is the $0.50/month line in the budget):

```bash
aws route53 create-hosted-zone --name yourdomain.com --caller-reference "$(date +%s)"
# Then, at your domain registrar, point its nameservers at the NS records this prints.
aws route53 change-resource-record-sets --hosted-zone-id <ZONE_ID> --change-batch '{
  "Changes": [{"Action": "CREATE", "ResourceRecordSet": {
    "Name": "api.yourdomain.com", "Type": "A", "TTL": 300,
    "ResourceRecords": [{"Value": "'"$PUBLIC_IP"'"}]
  }}]
}'
```

Caddy (step 11) needs this DNS record to resolve *before* it starts, so it can complete the
Let's Encrypt HTTP-01 challenge.

## 8. Server setup: Docker

```bash
ssh -i api-starter-key.pem ubuntu@$PUBLIC_IP
```

On the instance:

```bash
sudo apt-get update && sudo apt-get upgrade -y
curl -fsSL https://get.docker.com | sudo sh
sudo usermod -aG docker ubuntu
# Log out and back in for the group change to apply, or: newgrp docker

# Scenario B only (t3.nano has 0.5 GiB RAM; a swap file is cheap insurance against OOM
# kills when the app, Postgres, and Caddy are all warming up or under load at once).
sudo fallocate -l 1G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

## 9. Build and publish the image

On your local machine, from the project root, tagged for the GitHub Container Registry (free
for both public and private repos at this project's size — no ECR storage cost):

```bash
docker build -t ghcr.io/<your-github-username>/api-starter-axum:latest .

echo "<a GitHub Personal Access Token with write:packages>" | \
  docker login ghcr.io -u <your-github-username> --password-stdin

docker push ghcr.io/<your-github-username>/api-starter-axum:latest
```

If your GitHub repo is private, make the package's visibility match (or keep it private and
have the EC2 instance `docker login ghcr.io` too before pulling in step 12). The project's
existing `.github/workflows/ci.yml` already builds and tests on every push; adding a
build-and-push-to-GHCR job there is a natural next step once you're deploying regularly, but
isn't required to follow this manual — building locally and pushing by hand is enough to get
started.

## 10. Production `.env`

On the instance, in `~/api-starter-axum/` (create the directory), by hand (`scp` the repo's `.env.example` up as `.env` and fill in the blanks, or type the block below) — never commit this
file, and never have Claude or any other tool write your real secrets into version control:

```bash
mkdir -p ~/api-starter-axum && cd ~/api-starter-axum
nano .env
```

```bash
# --- Core ---
DATABASE_URL=postgres://postgres:postgres@db:5432/api_starter_db   # Scenario B: the `db` container, below
# DATABASE_URL=postgres://postgres:<DB_PASSWORD>@<rds-endpoint>:5432/api_starter_db  # Scenario A: RDS
BIND_ADDR=0.0.0.0:3000        # must be 0.0.0.0, not the 127.0.0.1 default — Caddy reaches
                               # the app over the Docker network, and 127.0.0.1 inside one
                               # container is not reachable from another
JWT_SECRET=<openssl rand -hex 32>
LOG_FORMAT=json

# --- Account & session security ---
FRONTEND_URL=https://api.yourdomain.com   # or your real frontend's origin, if separate
CORS_ALLOWED_ORIGINS=https://api.yourdomain.com
REQUIRE_VERIFIED_EMAIL=false
CHECK_PASSWORD_BREACHES=true

# --- First administrator ---
ADMIN_EMAIL=you@example.com
ADMIN_PASSWORD=<a strong password; rotate it after first login>

# --- Mail: leave disabled unless you've set up SES and accept its small extra cost ---
MAIL_ENABLED=false

# --- Uploads (profile photos and media) in S3 (see "Upload storage" below). No AWS keys here: on EC2 the
# SDK picks up the instance role automatically. ---
S3_BUCKET=api-starter-uploads-<a-unique-suffix>
AWS_REGION=us-east-1

# --- Metrics: leave unset. METRICS_BIND_ADDR is not published to the host or exposed
# publicly in the compose file below, so a token buys you little here at this budget. ---
```

Generate `JWT_SECRET` with `openssl rand -hex 32` (or copy it from a machine where you already
ran `cargo run -- gen --secrets`) — don't leave the placeholder in place.

### Upload storage (S3)

The API uploads profile photos and user media with the AWS SDK when `S3_BUCKET` is set, and serves them back
through itself, so the bucket stays private and needs no public-access settings. Create the
bucket and an instance role that can touch only the `avatars/` and `media/` prefixes inside it (run from your machine,
then attach the profile to the instance):

```bash
aws s3 mb s3://api-starter-uploads-<a-unique-suffix>
aws s3api put-public-access-block --bucket api-starter-uploads-<a-unique-suffix> \
  --public-access-block-configuration BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true

aws iam create-role --role-name api-starter-instance-role --assume-role-policy-document '{
  "Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]
}'
aws iam put-role-policy --role-name api-starter-instance-role --policy-name s3-uploads --policy-document '{
  "Version":"2012-10-17","Statement":[
    {"Effect":"Allow","Action":["s3:ListBucket"],"Resource":"arn:aws:s3:::api-starter-uploads-<a-unique-suffix>"},
    {"Effect":"Allow","Action":["s3:GetObject","s3:PutObject","s3:DeleteObject"],"Resource":["arn:aws:s3:::api-starter-uploads-<a-unique-suffix>/avatars/*","arn:aws:s3:::api-starter-uploads-<a-unique-suffix>/media/*"]}
  ]
}'
aws iam create-instance-profile --instance-profile-name api-starter-instance-profile
aws iam add-role-to-instance-profile --instance-profile-name api-starter-instance-profile --role-name api-starter-instance-role
aws ec2 associate-iam-instance-profile --instance-id "$INSTANCE_ID" \
  --iam-instance-profile Name=api-starter-instance-profile
```

`s3:ListBucket` is what the startup bucket check (`HeadBucket`) needs. The API refuses to start
if it cannot reach the bucket, so a wrong name or missing role shows up immediately in
`docker compose logs api`. The container reaches the instance role through the EC2 metadata
service; on Docker's default bridge network that works out of the box, but if you ever set
`--http-put-response-hop-limit 1` (IMDSv2 hop limit) on the instance, raise it to 2.

## 11. `docker-compose.prod.yml` and Caddy

Scenario B (self-hosted Postgres) — create `~/api-starter-axum/docker-compose.prod.yml`:

```yaml
services:
  api:
    image: ghcr.io/<your-github-username>/api-starter-axum:latest
    restart: unless-stopped
    env_file: .env
    depends_on:
      db:
        condition: service_healthy
    volumes:
      # no uploads volume: photos go to S3 (S3_BUCKET). Without S3, mount one at UPLOAD_DIR.

  db:
    image: postgres:16
    restart: unless-stopped
    environment:
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: api_starter_db
    volumes:
      - db_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 10

  caddy:
    image: caddy:2-alpine
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - caddy_data:/data
      - caddy_config:/config
    depends_on:
      - api

volumes:
  db_data:
  caddy_data:
  caddy_config:
```

Scenario A (RDS): delete the `db` service and its `depends_on`/`volumes` entries — `api` talks
straight to the RDS endpoint via `DATABASE_URL`.

`~/api-starter-axum/Caddyfile`:

```
api.yourdomain.com {
    reverse_proxy api:3000
}
```

Caddy fetches and renews the Let's Encrypt certificate for `api.yourdomain.com` automatically
the first time it starts, as long as the DNS `A` record from step 7 already resolves to this
instance and ports 80/443 are reachable (step 3).

## 12. First boot and verification

```bash
cd ~/api-starter-axum
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d
docker compose -f docker-compose.prod.yml logs -f api   # watch migrations run, then Ctrl-C
```

Migrations are embedded in the binary and run automatically at startup (see the main README) —
there is no separate migration step to run by hand.

From your own machine:

```bash
curl -s https://api.yourdomain.com/health/live
curl -s https://api.yourdomain.com/health/ready
curl -s -X POST https://api.yourdomain.com/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d "{\"email\":\"you@example.com\",\"password\":\"<ADMIN_PASSWORD>\"}"
```

Swagger UI is at `https://api.yourdomain.com/docs`.

## 13. Backups

**Scenario A**: RDS already takes daily automated backups with a 7-day retention window
(`--backup-retention-period 7` in step 5) at no extra cost within free tier — this alone is an
adequate backup story. The S3 export below is optional extra insurance.

**Scenario B**: you own backups entirely, since Postgres is just a container with a Docker
volume on one disk. Set up a daily `pg_dump` to S3:

```bash
aws s3 mb s3://api-starter-backups-<a-unique-suffix>
aws s3api put-bucket-lifecycle-configuration \
  --bucket api-starter-backups-<a-unique-suffix> \
  --lifecycle-configuration '{"Rules":[{"ID":"expire-old-backups","Status":"Enabled","Filter":{},"Expiration":{"Days":14}}]}'
```

Give the instance permission to write to just that bucket. The instance role and profile were
created in [10.](#10-production-env) for profile photos; this only adds a second policy to it
(an instance role, rather than copying long-lived IAM user credentials onto the server):

```bash
aws iam put-role-policy --role-name api-starter-instance-role --policy-name s3-backup-write --policy-document '{
  "Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["s3:PutObject"],"Resource":"arn:aws:s3:::api-starter-backups-<a-unique-suffix>/*"}]
}'
```

On the instance, install the AWS CLI and add a cron job:

```bash
sudo snap install aws-cli --classic   # or apt-get install awscli
crontab -e
```

```cron
0 3 * * * docker exec $(docker compose -f /home/ubuntu/api-starter-axum/docker-compose.prod.yml ps -q db) pg_dump -U postgres api_starter_db | gzip > /tmp/backup-$(date +\%F).sql.gz && aws s3 cp /tmp/backup-$(date +\%F).sql.gz s3://api-starter-backups-<a-unique-suffix>/ && rm /tmp/backup-$(date +\%F).sql.gz
```

## 14. Cost controls

Given a hard $30 ceiling, set an AWS Budget alert before you finish — this is the single most
important step for staying on budget, and creating it costs nothing:

```bash
aws budgets create-budget --account-id "$(aws sts get-caller-identity --query Account --output text)" \
  --budget '{
    "BudgetName": "api-starter-axum-6mo",
    "BudgetLimit": {"Amount": "30", "Unit": "USD"},
    "TimeUnit": "MONTHLY",
    "BudgetType": "COST"
  }' \
  --notifications-with-subscribers '[{
    "Notification": {"NotificationType": "ACTUAL", "ComparisonOperator": "GREATER_THAN", "Threshold": 80},
    "Subscribers": [{"SubscriptionType": "EMAIL", "Address": "you@example.com"}]
  }]'
```

(AWS Budgets tracks monthly spend, not a single 6-month total, so the practical version of this
is a $5/month budget with an 80% alert — adjust `BudgetLimit`/`Threshold` accordingly if you'd
rather be warned earlier.)

## 15. Redeploying updates

```bash
# Locally: rebuild and push a new image
docker build -t ghcr.io/<your-github-username>/api-starter-axum:latest .
docker push ghcr.io/<your-github-username>/api-starter-axum:latest

# On the instance: pull and recreate just the app container
cd ~/api-starter-axum
docker compose -f docker-compose.prod.yml pull api
docker compose -f docker-compose.prod.yml up -d api
```

New migrations run automatically the next time the `api` container starts.

## 16. Security hardening checklist

- SSH: key-only (the Ubuntu AMI ships this way by default) — confirm `PasswordAuthentication
  no` in `/etc/ssh/sshd_config`.
- Security group port 22 is restricted to your IP (step 3) — re-tighten it whenever your IP
  changes; don't leave it open to `0.0.0.0/0`.
- `.env` is never committed and is only ever created by hand on the server (this mirrors the
  same rule the main README documents for local development).
- `sudo apt-get upgrade -y` periodically (there's no unattended-upgrades daemon configured here
  to keep this manual's moving parts minimal — add one if you want it automated).
- Rotate `ADMIN_PASSWORD` after first login, and rotate `JWT_SECRET` (which invalidates every
  existing session) if you ever suspect it leaked.

## 17. Monitoring within the RAM budget

A self-hosted Prometheus + Grafana stack (the project ships `docker-compose.monitoring.yml` for
local use) is not run here — on a 0.5–1 GiB instance it would compete with the app and Postgres
for memory that isn't there to spare. Instead:

- `GET /health/live` and `GET /health/ready` are the two endpoints to actually watch.
- A free external uptime checker (e.g. one that polls `/health/live` every few minutes) gives
  you outage alerts without running anything on the instance at all.
- `/metrics` still works and is still useful for ad hoc debugging — `ssh` in and `curl
  localhost:9091/metrics` directly, since it's not published to the host or the internet.

## Known limits and tradeoffs

Deliberate consequences of fitting this deployment into $30/6 months — not oversights:

- No high availability: a single instance restarting (a patch, a reboot, an AWS host
  maintenance event) means brief downtime. There is no load balancer or second instance to fail
  over to.
- Scenario B's Postgres has no automated backup system beyond the cron job in
  [13.](#13-backups) — you are your own DBA. Scenario A (RDS) does not have this gap.
- No autoscaling: a genuine traffic spike degrades or falls over rather than scaling out: a
  `t3.nano`/`t3.micro` has one shared vCPU.
- No self-hosted metrics dashboard, for RAM reasons — see [17.](#17-monitoring-within-the-ram-budget).
- The rate limiter and profile-photo storage are already documented as per-instance limitations
  in the main README; they remain true here since this is still a single instance.
- This manual optimizes for staying inside a hard, small budget, not for being the "correct"
  production topology for a real product with paying users. Once traffic or reliability
  requirements grow past what a single `t3` instance can absorb, the right next step is
  reintroducing the pieces skipped here (a load balancer, RDS Multi-AZ, autoscaling) — not
  squeezing more onto one box.

## Teardown after 6 months

To stop spending once the budget window is up:

```bash
docker compose -f docker-compose.prod.yml down
aws ec2 terminate-instances --instance-ids "$INSTANCE_ID"
aws ec2 release-address --allocation-id "$ALLOC_ID"
aws rds delete-db-instance --db-instance-identifier api-starter-db --skip-final-snapshot   # Scenario A only
aws s3 rb s3://api-starter-backups-<a-unique-suffix> --force
aws route53 delete-hosted-zone --id <ZONE_ID>   # if you created one
```

Double-check `aws ec2 describe-addresses` and `aws ec2 describe-instances` afterward — a
leftover unattached Elastic IP is the most common way people keep paying after they think
they've torn everything down.
