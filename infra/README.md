# FBKL infrastructure (OpenTofu)

Provisions the serverless deploy (epic `fbkl-rust-96e`) across AWS and
Cloudflare, with Postgres hosted externally on Supabase. Uses **OpenTofu** (`tofu`) — the `.tf` code, state, and lock file are
Terraform-compatible, so the HashiCorp docs apply; only the CLI name differs.
State lives in S3 (`fbkl-tfstate-820712214931`), region `us-east-1`, authenticated
via the **`fbkl`** SSO profile (personal account, never the work default).

## Prerequisites (install once)

| Tool | Install | Why |
|------|---------|-----|
| OpenTofu `tofu` | `brew install opentofu` | runs this config |
| AWS CLI v2 | `brew install awscli` | SSO login + ad-hoc checks |
| cargo-lambda | `brew tap cargo-lambda/cargo-lambda && brew install cargo-lambda` *(or `cargo install cargo-lambda`)* | cross-compiles the Rust Lambdas to arm64 zips that `lambdas.tf` deploys |
| Zig | bundled with recent cargo-lambda; else `brew install zig` | cargo-lambda's cross-linker |

## Build the Lambda artifacts (before any apply that touches functions)

`lambdas.tf` deploys zips produced by cargo-lambda. Build them from the repo root:

```bash
cargo lambda build --release --arm64 --output-format zip
# -> target/lambda/{fbkl-api,fbkl-scheduler,fbkl-session-gc}/bootstrap.zip
```

If the zips are missing, `tofu validate` still passes (hash is guarded), but
`tofu apply` will fail — build first.

## First-time setup

```bash
# 0. Authenticate the personal account
aws sso login --profile fbkl

# 1. Create the state bucket (local state, run once)  [DONE]
cd infra/bootstrap
tofu init
tofu apply
cd ..

# 2. Init the main config against that bucket  [DONE]
tofu init

# 3. Build Lambda zips, then review + apply
cargo lambda build --release --arm64 --output-format zip   # from repo root
tofu plan
tofu apply
```

Apply needs these inputs (pass via env, never commit — keep in `infra/secrets.env`):

```bash
export TF_VAR_supabase_database_url='postgresql://…pooler.supabase.com:6543/postgres'  # Supabase TRANSACTION pooler → Lambda FBKL_DATABASE_URL
export CLOUDFLARE_API_TOKEN='…'                    # Account · Cloudflare Pages · Edit
export TF_VAR_cloudflare_account_id='…'            # not secret; dashboard sidebar
```

The DB lives on Supabase (created in the Supabase dashboard, not managed here);
its transaction-pooler URL is passed in via `TF_VAR_supabase_database_url`. The
session secret and SPA origin are NOT inputs — `secrets.tf` generates a stable
session secret, and the API CORS origin is sourced from the logged-in app's
Pages subdomain.

## Run migrations (after the Supabase project exists, before the API serves traffic)

Migrations use the SESSION pooler (port 5432); the transaction pooler breaks DDL
+ advisory locks. The sea-orm migration crate reads `DATABASE_URL` (distinct from
the app's `FBKL_DATABASE_URL`):

```bash
# from repo root — Supabase dashboard → Connect → "Session pooler"
export DATABASE_URL='postgresql://…pooler.supabase.com:5432/postgres'

cargo run -p fbkl-migration -- up                       # app schema (sea-orm)
cargo run -p fbkl-server --bin migrate_sessions         # tower_sessions table
```

`migrate_sessions` runs tower-sessions' `PostgresStore::migrate()` against the
session pooler — the serverless equivalent of what the local dev bin does on
startup. Both commands read `DATABASE_URL` (the SESSION pooler). In CI the same
value comes from the `PROD_DATABASE_MIGRATION_URL` GitHub secret.

## Files

| File | Purpose | bd task |
|------|---------|---------|
| `bootstrap/main.tf` | S3 state bucket (run once, local state) | — |
| `providers.tf` | Terraform + AWS provider, S3 backend | — |
| `variables.tf` | region/profile/repo/app_origin inputs | — |
| `secrets.tf` | generated stable session secret | 96e.3 |
| `github_oidc.tf` | GitHub Actions OIDC provider + deploy role | 96e.10 |
| `lambdas.tf` | 3 functions, exec role, Function URL + CORS, concurrency | 96e.3/.4/.5 |
| `eventbridge.tf` | scheduler (1-min) + session-gc (5-min) schedules + invoke role | 96e.7 |
| `cloudflare.tf` | Pages projects for both Vite apps (*.pages.dev) | 96e.8 |
| `observability.tf` | CloudWatch error/throttle alarms + SNS email | 96e.9 |

Secrets (`terraform.tfvars`, `*.tfstate`) are gitignored. The deploy role ARN is
emitted as the `github_deploy_role_arn` output for the CI workflow.

## CI/CD (`.github/workflows/deploy.yml`, 96e.10)

Push to `main` triggers a path-filtered deploy: gated migrations → Lambda code
update (OIDC, no stored keys) → Cloudflare Pages deploy (wrangler). Configure in
the GitHub repo before the first CI run:

| Kind | Name | Value |
|------|------|-------|
| Variable | `AWS_DEPLOY_ROLE_ARN` | `tofu -chdir=infra output -raw github_deploy_role_arn` |
| Variable | `AWS_REGION` | `us-east-1` |
| Variable | `CLOUDFLARE_ACCOUNT_ID` | your account ID |
| Secret | `PROD_DATABASE_MIGRATION_URL` | Supabase SESSION pooler URL (port 5432) |
| Secret | `CLOUDFLARE_API_TOKEN` | Pages-edit token |
| Environment | `production` | add required reviewers → this is the migration approval gate |
