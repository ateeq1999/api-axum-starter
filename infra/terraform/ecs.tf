locals {
  api_container_port = 3000

  # A blank value means "not set" to the API (see .env.example), and ECS rejects blank
  # environment values anyway, so blanks are dropped rather than passed through.
  api_environment = {
    BIND_ADDR              = "0.0.0.0:${local.api_container_port}"
    LOG_FORMAT             = "json"
    FRONTEND_URL           = var.frontend_url
    CORS_ALLOWED_ORIGINS   = var.cors_allowed_origins
    REQUIRE_VERIFIED_EMAIL = tostring(var.require_verified_email)
    S3_BUCKET              = aws_s3_bucket.uploads.bucket
    AWS_DEFAULT_REGION     = var.aws_region
    ADMIN_EMAIL            = var.admin_email
    MAIL_ENABLED           = tostring(var.smtp_host != "")
    SMTP_HOST              = var.smtp_host
    SMTP_PORT              = tostring(var.smtp_port)
    SMTP_TLS               = "none"
    MAIL_FROM              = "Starter <no-reply@starter.example.com>"
  }

  api_environment_list = [
    for name, value in local.api_environment : { name = name, value = value } if value != ""
  ]

  # The API needs both ADMIN_EMAIL and ADMIN_PASSWORD or neither.
  api_secrets_list = concat(
    [
      { name = "JWT_SECRET", valueFrom = aws_secretsmanager_secret.jwt_signing_secret.arn },
      { name = "DATABASE_URL", valueFrom = aws_secretsmanager_secret.database_url.arn },
    ],
    var.admin_email != "" ? [
      { name = "ADMIN_PASSWORD", valueFrom = aws_secretsmanager_secret.admin_password.arn },
    ] : [],
  )

  api_image = "${aws_ecr_repository.api.repository_url}:${var.container_image_tag}"
}

resource "aws_cloudwatch_log_group" "api" {
  name              = "/ecs/${var.project_name}-axum"
  retention_in_days = 7
}

resource "aws_ecs_cluster" "api" {
  name = "${var.project_name}-cluster"
}

# floci publishes a task's port on your machine only for bridge networking with an explicit
# hostPort (awsvpc tasks stay reachable only from inside the Docker network), hence the EC2
# launch type and bridge mode rather than the Fargate default.
resource "aws_ecs_task_definition" "api" {
  family                   = "${var.project_name}-axum"
  requires_compatibilities = ["EC2"]
  network_mode             = "bridge"
  cpu                      = var.task_cpu_units
  memory                   = var.task_memory_mib
  execution_role_arn       = aws_iam_role.task_execution.arn
  task_role_arn            = aws_iam_role.task.arn

  container_definitions = jsonencode([
    {
      name      = "api"
      image     = local.api_image
      essential = true

      portMappings = [
        {
          containerPort = local.api_container_port
          hostPort      = var.api_host_port
          protocol      = "tcp"
        },
      ]

      environment = local.api_environment_list
      secrets     = local.api_secrets_list

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.api.name
          "awslogs-region"        = var.aws_region
          "awslogs-stream-prefix" = "api"
        }
      }
    },
  ])
}

resource "aws_ecs_service" "api" {
  name            = "${var.project_name}-axum"
  cluster         = aws_ecs_cluster.api.id
  task_definition = aws_ecs_task_definition.api.arn
  desired_count   = 1
  launch_type     = "EC2"
}
