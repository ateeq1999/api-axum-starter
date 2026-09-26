# Every AWS call goes to the floci emulator instead of real AWS. floci accepts any credentials,
# so the fixed "test" pair below is not a secret. Only the services this stack uses need an
# endpoint override.
provider "aws" {
  region     = var.aws_region
  access_key = "test"
  secret_key = "test"

  # floci is not AWS: there is no real account to validate credentials or look up an account id.
  skip_credentials_validation = true
  skip_metadata_api_check     = true
  skip_requesting_account_id  = true
  skip_region_validation      = true

  # Emulators serve buckets as `endpoint/bucket/key`, not `bucket.endpoint/key`.
  s3_use_path_style = true

  endpoints {
    ec2            = var.floci_endpoint
    ecr            = var.floci_endpoint
    ecs            = var.floci_endpoint
    iam            = var.floci_endpoint
    logs           = var.floci_endpoint
    rds            = var.floci_endpoint
    s3             = var.floci_endpoint
    secretsmanager = var.floci_endpoint
    sts            = var.floci_endpoint
  }
}
