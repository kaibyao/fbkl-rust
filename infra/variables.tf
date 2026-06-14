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

variable "fbkl_database_url" {
  type        = string
  sensitive   = true
  description = "Neon POOLED (-pooler / PgBouncer transaction mode) connection string for the Lambdas. Produced by 96e.6; pass via TF_VAR_fbkl_database_url, never commit."
}

variable "session_secret" {
  type        = string
  sensitive   = true
  description = "Secret used to derive the tower-sessions private-cookie key. Pass via TF_VAR_session_secret; keep stable across deploys so existing sessions survive."
}

variable "app_origin" {
  type        = string
  description = "Exact origin of the logged-in SPA on Cloudflare Pages (e.g. https://app.example.com). Used for the API Function URL CORS allow-list with credentials."
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
