# Bootstrap: creates the S3 bucket that stores the MAIN config's Terraform state.
#
# Chicken-and-egg: the state bucket must exist before the main config can use it
# as a backend. This tiny config uses LOCAL state (committed nowhere) to create
# just that one bucket, then you never touch it again.
#
# Usage (run once):
#   cd infra/bootstrap
#   terraform init
#   terraform apply
# Then `cd ..` and `terraform init` the main config against this bucket.

terraform {
  required_version = ">= 1.10"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = "us-east-1"
  # Always scope to the personal account profile — never the machine default
  # (which is a work account).
  profile = "fbkl"
}

resource "aws_s3_bucket" "tfstate" {
  bucket = "fbkl-tfstate-820712214931"

  # Safety: prevent accidental `terraform destroy` from deleting state history.
  lifecycle {
    prevent_destroy = true
  }
}

# Keep every version of the state file so a bad apply can be rolled back.
resource "aws_s3_bucket_versioning" "tfstate" {
  bucket = aws_s3_bucket.tfstate.id
  versioning_configuration {
    status = "Enabled"
  }
}

# State contains secrets (DB URLs, generated passwords) — encrypt at rest.
resource "aws_s3_bucket_server_side_encryption_configuration" "tfstate" {
  bucket = aws_s3_bucket.tfstate.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# Belt-and-suspenders: block all public access to the state bucket.
resource "aws_s3_bucket_public_access_block" "tfstate" {
  bucket                  = aws_s3_bucket.tfstate.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

output "state_bucket" {
  value       = aws_s3_bucket.tfstate.id
  description = "Bucket name to use in the main config's backend block."
}
