# floci ships a default VPC with one subnet per availability zone, the same as a fresh AWS
# account, so this stack uses it rather than creating a network of its own.
data "aws_vpc" "default" {
  default = true
}

data "aws_subnets" "default" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

resource "aws_security_group" "database" {
  name        = "${var.project_name}-database"
  description = "Postgres, reachable from inside the VPC only"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    description = "Postgres from the VPC"
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = [data.aws_vpc.default.cidr_block]
  }
}
