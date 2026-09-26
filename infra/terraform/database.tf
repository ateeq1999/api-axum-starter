# floci runs each RDS instance as a real Postgres container, so migrations, advisory locks and
# everything else the API relies on behave exactly as on a real server.
resource "aws_db_subnet_group" "api" {
  name       = "${var.project_name}-database"
  subnet_ids = data.aws_subnets.default.ids
}

resource "aws_db_instance" "api" {
  identifier             = "${var.project_name}-database"
  engine                 = "postgres"
  engine_version         = "16"
  instance_class         = var.database_instance_class
  allocated_storage      = var.database_allocated_storage_gb
  db_name                = "api_starter_db"
  username               = "postgres"
  password               = random_password.database.result
  db_subnet_group_name   = aws_db_subnet_group.api.name
  vpc_security_group_ids = [aws_security_group.database.id]
  publicly_accessible    = false
  skip_final_snapshot    = true
}
