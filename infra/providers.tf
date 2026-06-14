terraform {
  required_version = ">= 1.10"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    neon = {
      source  = "kislerdm/neon"
      version = "~> 0.9"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.6"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 5.0"
    }
  }

  # Remote state in the bucket created by infra/bootstrap.
  # Native S3 locking (use_lockfile) replaces the old DynamoDB lock table (TF 1.10+).
  backend "s3" {
    bucket       = "fbkl-tfstate-820712214931"
    key          = "fbkl/terraform.tfstate"
    region       = "us-east-1"
    profile      = "fbkl"
    encrypt      = true
    use_lockfile = true
  }
}

provider "aws" {
  region = var.aws_region
  # Always the personal-account SSO profile; never the machine default.
  profile = var.aws_profile

  default_tags {
    tags = {
      Project   = "fbkl"
      ManagedBy = "terraform"
    }
  }
}
