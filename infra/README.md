# FBKL infrastructure (OpenTofu)

Provisions the serverless deploy (epic `fbkl-rust-96e`) across AWS, Neon, and
Cloudflare. Uses **OpenTofu** (`tofu`) — the `.tf` code, state, and lock file are
Terraform-compatible, so the HashiCorp docs apply; only the CLI name differs.
State lives in S3 (`fbkl-tfstate-820712214931`), region `us-east-1`, authenticated
via the **`fbkl`** SSO profile (personal account, never the work default).

## First-time setup

```bash
# 0. Authenticate the personal account
aws sso login --profile fbkl

# 1. Create the state bucket (local state, run once)
cd infra/bootstrap
tofu init
tofu apply
cd ..

# 2. Init the main config against that bucket
tofu init

# 3. Review + apply
tofu plan
tofu apply
```

## Files

| File | Purpose | bd task |
|------|---------|---------|
| `bootstrap/main.tf` | S3 state bucket (run once, local state) | — |
| `providers.tf` | Terraform + AWS provider, S3 backend | — |
| `variables.tf` | region/profile/repo inputs | — |
| `github_oidc.tf` | GitHub Actions OIDC provider + deploy role | 96e.10 |
| `lambdas.tf` *(todo)* | 3 functions, Function URL + CORS, concurrency | 96e.3 |
| `eventbridge.tf` *(todo)* | scheduler (1-min) + session-gc (5-min) | 96e.7 |
| `observability.tf` *(todo)* | CloudWatch alarms | 96e.9 |
| `neon.tf` *(todo)* | Neon project, pooled + direct endpoints | 96e.6 |
| `cloudflare.tf` *(todo)* | Pages projects for both Vite apps | 96e.8 |

Secrets (`terraform.tfvars`, `*.tfstate`) are gitignored. The deploy role ARN is
emitted as the `github_deploy_role_arn` output for the CI workflow.
