# Generated once and kept in Terraform state. Alphanumeric only, so the database password is safe
# to embed in a connection URL without escaping.
resource "random_password" "jwt_signing_secret" {
  length  = 64
  special = false
}

resource "random_password" "database" {
  length  = 24
  special = false
}

resource "random_password" "admin" {
  length  = 24
  special = false
}

locals {
  # The password you chose (admin_password), or the generated one when you left it blank.
  admin_password = var.admin_password != "" ? var.admin_password : random_password.admin.result
}

# The values the API must not have in plain text in its task definition. ECS reads them from
# Secrets Manager when it starts the container.
resource "aws_secretsmanager_secret" "jwt_signing_secret" {
  name                    = "${var.project_name}/jwt-signing-secret"
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "jwt_signing_secret" {
  secret_id     = aws_secretsmanager_secret.jwt_signing_secret.id
  secret_string = random_password.jwt_signing_secret.result
}

resource "aws_secretsmanager_secret" "database_url" {
  name                    = "${var.project_name}/database-url"
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "database_url" {
  secret_id     = aws_secretsmanager_secret.database_url.id
  secret_string = "postgres://${aws_db_instance.api.username}:${random_password.database.result}@${aws_db_instance.api.endpoint}/${aws_db_instance.api.db_name}"
}

resource "aws_secretsmanager_secret" "admin_password" {
  name                    = "${var.project_name}/admin-password"
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "admin_password" {
  secret_id     = aws_secretsmanager_secret.admin_password.id
  secret_string = local.admin_password
}
