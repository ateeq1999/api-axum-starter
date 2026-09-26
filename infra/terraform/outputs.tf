output "api_url" {
  description = "The running API, published on your machine."
  value       = "http://localhost:${var.api_host_port}"
}

output "api_docs_url" {
  description = "Swagger UI."
  value       = "http://localhost:${var.api_host_port}/docs"
}

output "container_image" {
  description = "The image reference the ECS service runs."
  value       = local.api_image
}

output "ecr_repository_url" {
  description = "Where deploy.sh pushes the image, as floci reports it."
  value       = aws_ecr_repository.api.repository_url
}

output "uploads_bucket" {
  description = "S3 bucket holding avatars/ and media/."
  value       = aws_s3_bucket.uploads.bucket
}

output "database_endpoint" {
  description = "Host and port of the RDS Postgres instance (reachable from inside the Docker network)."
  value       = aws_db_instance.api.endpoint
}

output "admin_email" {
  description = "Bootstrap administrator's email."
  value       = var.admin_email
}

output "admin_password" {
  description = "Bootstrap administrator's password. Show it with: ./tf.sh output -raw admin_password"
  value       = random_password.admin.result
  sensitive   = true
}
