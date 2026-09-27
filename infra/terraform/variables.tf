variable "floci_endpoint" {
  description = "Base URL of the floci emulator as seen from wherever Terraform runs. tf.sh sets this for you (http://floci:4566 from inside the Docker network)."
  type        = string
  default     = "http://localhost:4566"
}

variable "aws_region" {
  description = "Region to create resources in. floci defaults to us-east-1."
  type        = string
  default     = "us-east-1"
}

variable "project_name" {
  description = "Prefix for every resource name."
  type        = string
  default     = "api-starter"
}

variable "container_image_tag" {
  description = "Tag of the API image in the ECR repository that the ECS service runs."
  type        = string
  default     = "latest"
}

variable "api_host_port" {
  description = "Port on your machine that the API is published on (http://localhost:<port>)."
  type        = number
  default     = 3000
}

variable "frontend_url" {
  description = "Public URL of your frontend. Used for links in emails and as the default CORS origin."
  type        = string
  default     = "http://localhost:3001"
}

variable "cors_allowed_origins" {
  description = "Comma-separated browser origins allowed to call the API. Blank means just frontend_url."
  type        = string
  default     = ""
}

variable "admin_email" {
  description = "Bootstrap administrator created on first start. Blank creates none."
  type        = string
  default     = "admin@example.com"
}

variable "admin_password" {
  description = "Password for the bootstrap administrator. Blank generates a random one, which deploy.sh prints when it finishes. deploy.sh passes it as TF_VAR_admin_password so it never appears on a command line."
  type        = string
  default     = ""
  sensitive   = true

  validation {
    condition     = var.admin_password == "" || (length(var.admin_password) >= 8 && length(var.admin_password) <= 128)
    error_message = "admin_password must be 8-128 characters (the API's password rule)."
  }
}

variable "require_verified_email" {
  description = "Block password login until the email is verified."
  type        = bool
  default     = false
}

variable "smtp_host" {
  description = "SMTP server the API sends mail through. The default is the Mailpit container on the shared infra network (web UI http://localhost:8025). Blank turns mail sending off (emails are only logged)."
  type        = string
  default     = "infra-mailpit"
}

variable "smtp_port" {
  description = "SMTP port. Mailpit listens on 1025."
  type        = number
  default     = 1025
}

variable "database_instance_class" {
  description = "RDS instance class."
  type        = string
  default     = "db.t3.micro"
}

variable "database_allocated_storage_gb" {
  description = "RDS storage in GB."
  type        = number
  default     = 5
}

variable "task_cpu_units" {
  description = "CPU units reserved for the API container (1024 = one vCPU)."
  type        = number
  default     = 256
}

variable "task_memory_mib" {
  description = "Memory reserved for the API container. Password hashing can use ~19 MiB per concurrent hash (MAX_CONCURRENT_HASHES), so keep this at 512 or more."
  type        = number
  default     = 512
}
