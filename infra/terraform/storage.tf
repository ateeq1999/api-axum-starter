# Where the API keeps profile photos (avatars/...) and user media (media/...). The API serves
# files back itself, so the bucket stays private.
resource "aws_s3_bucket" "uploads" {
  bucket = "${var.project_name}-uploads"

  # Lets `./tf.sh destroy` remove the bucket even when it holds uploads. This stack is for local
  # testing; a real deployment should not set this.
  force_destroy = true
}

resource "aws_s3_bucket_public_access_block" "uploads" {
  bucket = aws_s3_bucket.uploads.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}
