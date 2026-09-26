# Where the API image lives. deploy.sh builds the project's Dockerfile and pushes it here before
# the ECS service is created, so the service never starts without an image.
resource "aws_ecr_repository" "api" {
  name         = "${var.project_name}-axum"
  force_delete = true

  image_scanning_configuration {
    scan_on_push = false
  }
}
