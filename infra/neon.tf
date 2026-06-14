# Neon managed Postgres (96e.6).
#
# Authenticated via the NEON_API_KEY env var (set it before plan/apply).
# Region matches the Lambda region (us-east-1) to minimize round-trip latency.
#
# Two endpoints come out of one project:
#   - connection_uri_pooler -> Lambdas (PgBouncer transaction mode). Wired into
#     FBKL_DATABASE_URL in lambdas.tf. Pooling is what makes Lambda's per-invoke
#     connection churn survivable.
#   - connection_uri (direct) -> schema migrations only. The pooler breaks DDL
#     and advisory locks, so migrations MUST use the direct endpoint.

provider "neon" {
  # api_key read from NEON_API_KEY
}

resource "neon_project" "fbkl" {
  name       = "fbkl"
  region_id  = "aws-us-east-1"
  pg_version = 17

  branch {
    name          = "main"
    database_name = "fbkl"
    role_name     = "fbkl"
  }

  # Free-tier-friendly compute: scale down to 0.25 CU, cap at 1 CU.
  default_endpoint_settings {
    autoscaling_limit_min_cu = 0.25
    autoscaling_limit_max_cu = 1
  }
}

# Pooled URI for the app Lambdas (consumed in lambdas.tf).
output "neon_database_url_pooled" {
  value       = neon_project.fbkl.connection_uri_pooler
  sensitive   = true
  description = "POOLED connection string. App runtime (FBKL_DATABASE_URL). View with: tofu output -raw neon_database_url_pooled"
}

# Direct URI for running migrations (sea-orm migration crate reads DATABASE_URL).
# Run from repo root before/with each deploy:
#   DATABASE_URL=$(tofu -chdir=infra output -raw neon_database_url_direct) cargo run -p fbkl-migration -- up
output "neon_database_url_direct" {
  value       = neon_project.fbkl.connection_uri
  sensitive   = true
  description = "DIRECT (non-pooled) connection string. Use ONLY for migrations (DDL + advisory locks)."
}
