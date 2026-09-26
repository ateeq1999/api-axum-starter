data "aws_iam_policy_document" "ecs_tasks_assume_role" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

# Used by ECS itself to start the container: pull the image, write logs, read the secrets below.
resource "aws_iam_role" "task_execution" {
  name               = "${var.project_name}-task-execution"
  assume_role_policy = data.aws_iam_policy_document.ecs_tasks_assume_role.json
}

data "aws_iam_policy_document" "task_execution" {
  statement {
    actions = [
      "ecr:GetAuthorizationToken",
      "ecr:BatchGetImage",
      "ecr:GetDownloadUrlForLayer",
      "logs:CreateLogStream",
      "logs:PutLogEvents",
    ]
    resources = ["*"]
  }

  statement {
    actions = ["secretsmanager:GetSecretValue"]
    resources = [
      aws_secretsmanager_secret.jwt_signing_secret.arn,
      aws_secretsmanager_secret.database_url.arn,
      aws_secretsmanager_secret.admin_password.arn,
    ]
  }
}

resource "aws_iam_role_policy" "task_execution" {
  name   = "start-the-container"
  role   = aws_iam_role.task_execution.id
  policy = data.aws_iam_policy_document.task_execution.json
}

# Used by the running API: nothing but its own slice of the uploads bucket.
resource "aws_iam_role" "task" {
  name               = "${var.project_name}-task"
  assume_role_policy = data.aws_iam_policy_document.ecs_tasks_assume_role.json
}

data "aws_iam_policy_document" "task" {
  # HeadBucket at startup is what checks the bucket is reachable.
  statement {
    actions   = ["s3:ListBucket"]
    resources = [aws_s3_bucket.uploads.arn]
  }

  statement {
    actions = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"]
    resources = [
      "${aws_s3_bucket.uploads.arn}/avatars/*",
      "${aws_s3_bucket.uploads.arn}/media/*",
    ]
  }
}

resource "aws_iam_role_policy" "task" {
  name   = "uploads-bucket"
  role   = aws_iam_role.task.id
  policy = data.aws_iam_policy_document.task.json
}
