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

# Supabase pooled URL for the Lambda runtime. Use the SESSION pooler (port 5432):
# the transaction pooler (6543) shares backends between client connections, and
# sqlx's per-connection sqlx_s_N statement names then collide across Lambda
# execution environments. Migrations use their own SESSION pooler URL, set as the
# PROD_DATABASE_MIGRATION_URL GitHub secret, not here.
variable "supabase_database_url" {
  type        = string
  sensitive   = true
  description = "Supabase SESSION pooler connection string (port 5432) for the Lambda runtime FBKL_DATABASE_URL."
}

# NOTE: the session secret is not an input — secrets.tf generates a stable one,
# wired directly into the Lambda env in lambdas.tf.

variable "cloudflare_account_id" {
  type        = string
  description = "Cloudflare account ID that owns the Pages projects. Not secret; find it in the dashboard sidebar."
}

variable "alert_email" {
  type        = string
  default     = "ohkaiby@gmail.com"
  description = "Email address that receives CloudWatch alarm notifications via SNS. Must confirm the subscription email."
}

# The Supabase session pooler holds one backend per client connection for that
# connection's life, so its pool size (15) — not the instance's 60 direct
# connections — is the ceiling. Each warm execution env keeps one backend
# (max_connections(1) in lambdas/src/lib.rs) until it idles out, so peak held
# backends = peak concurrency: 10 api + 2 scheduler + 2 session-gc = 14, leaving
# one for a CI migration run. Raising this means raising Pool Size in the Supabase
# dashboard first (Settings -> Database -> Connection pooling).
variable "api_reserved_concurrency" {
  type        = number
  default     = 10
  description = "Reserved concurrency cap on the API Lambda — bounds worst-case backends held on the Supabase session pooler (pool size 15)."
}

variable "worker_reserved_concurrency" {
  type        = number
  default     = 2
  description = "Reserved concurrency for the scheduler + session-gc Lambdas (one tick at a time is fine)."
}
