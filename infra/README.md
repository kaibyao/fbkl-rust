# FBKL infrastructure (OpenTofu)

Provisions the serverless deploy (epic `fbkl-rust-96e`) across AWS, Neon, and
Cloudflare. Uses **OpenTofu** (`tofu`) — the `.tf` code, state, and lock file are
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

Apply needs these inputs (pass via env, never commit):

```bash
export NEON_API_KEY='napi_…'                          # provisions Neon (neon.tf)
export TF_VAR_app_origin='https://app.example.com'    # 96e.8, SPA origin for CORS
```

The DB connection string and session secret are NOT inputs — `neon.tf`
provisions the database and `secrets.tf` generates a stable session secret;
both are wired straight into the Lambda env.

## Run migrations (after Neon exists, before the API serves traffic)

Migrations use the DIRECT endpoint (the pooler breaks DDL + advisory locks). The
sea-orm migration crate reads `DATABASE_URL` (distinct from the app's
`FBKL_DATABASE_URL`):

```bash
# from repo root, after `tofu apply` has created the Neon project
DATABASE_URL=$(tofu -chdir=infra output -raw neon_database_url_direct) \
  cargo run -p fbkl-migration -- up
```

The tower-sessions table is created by the app's `session_store.migrate()` on the
local bin; for the serverless DB, run it as part of this one-time migration step.

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
| `neon.tf` | Neon project, pooled + direct endpoints | 96e.6 |
| `observability.tf` *(todo)* | CloudWatch alarms | 96e.9 |
| `cloudflare.tf` *(todo)* | Pages projects for both Vite apps | 96e.8 |

Secrets (`terraform.tfvars`, `*.tfstate`) are gitignored. The deploy role ARN is
emitted as the `github_deploy_role_arn` output for the CI workflow.
