variable "aws_region" {
  type        = string
  default     = "us-east-1"
  description = "AWS region for all resources (Lambda free tier applies in any region)."
}

variable "aws_profile" {
  type        = string
  default     = "fbkl"
  description = "Local AWS CLI/SSO profile to authenticate with. MUST be the personal fbkl account, not the machine default."
}

variable "github_repo" {
  type        = string
  default     = "kaibyao/fbkl-rust"
  description = "owner/repo allowed to assume the CI deploy role via GitHub OIDC."
}

variable "github_deploy_ref" {
  type        = string
  default     = "refs/heads/main"
  description = "Git ref permitted to assume the deploy role. Restricts CI deploys to this branch."
}

# NOTE: the Lambda DB URL and session secret are no longer inputs — they are
# produced inside this config (neon.tf provisions the DB; secrets.tf generates a
# stable session secret) and wired directly into the Lambda env in lambdas.tf.

variable "cloudflare_account_id" {
  type        = string
  description = "Cloudflare account ID that owns the Pages projects. Not secret; find it in the dashboard sidebar."
}

variable "alert_email" {
  type        = string
  default     = "ohkaiby@gmail.com"
  description = "Email address that receives CloudWatch alarm notifications via SNS. Must confirm the subscription email."
}

variable "api_reserved_concurrency" {
  type        = number
  default     = 50
  description = "Reserved concurrency cap on the API Lambda — the backstop that bounds worst-case client connections to the Neon pooler."
}

variable "worker_reserved_concurrency" {
  type        = number
  default     = 2
  description = "Reserved concurrency for the scheduler + session-gc Lambdas (one tick at a time is fine)."
}
