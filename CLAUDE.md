# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

FBKL is a custom fantasy basketball league system built as a full-stack monorepo. The backend uses Rust with GraphQL/SeaORM, and the frontend uses React/Vite/TypeScript. The system simulates NBA team management with player contracts, trades, auctions, and draft picks.

## Tech Stack

### Backend (Rust)
- **Language**: Rust (Edition 2024)
- **Web framework**: Axum
- **API**: GraphQL via async-graphql
- **Database ORM**: SeaORM (type-safe queries)
- **Auth**: Custom auth crate, Argon2 password hashing
- **Package manager**: Cargo (workspace)

### Frontend (TypeScript/React)
- **Build tool**: Vite (both apps)
- **Routing (logged-in app)**: TanStack Router
- **UI**: shadcn/ui backed by Base UI, styled with Tailwind CSS v4 (preset `bzWLdGMr4`, style `base-lyra`, zinc base, orange accent, Nunito Sans Variable headings + Figtree Variable body, Lucide icons). MUI v7 still present and being phased out.
- **GraphQL client**: urql
- **Forms**: react-hook-form
- **Lint/format**: oxlint + oxfmt
- **Type generation**: GraphQL Code Generator
- **Package manager**: pnpm v10.11.1 (workspace)

## Architecture

This is a Rust workspace with multiple crates that work together:

### Backend (Rust)
- **`server/`** - Main GraphQL API server with authentication. Contains no business logic - delegates to `logic/`
- **`entity/`** - Database models and queries using SeaORM
- **`logic/`** - Core business logic for trades, auctions, roster management, IR, rookie development
- **`migration/`** - Database schema migrations using SeaORM CLI
- **`auth/`** - Authentication and authorization logic
- **`constants/`** - Shared constants across the workspace
- **`jobs/`** - Background job processing
- **`transaction-processor/`** - Processes league transactions and maintains history
- **`import-data/`** - Data import utilities for ESPN NBA data
- **`graphql-generation/`** - Generates GraphQL schema for frontend type generation

### Frontend (React/TypeScript)
- **`webapp-logged-in/`** - Vite app for authenticated users (port 9100). shadcn/ui components live in `src/components/ui` (aliased as `@/components/ui`); theme tokens in `src/global.css`
- **`webapp-public/`** - Vite app for public users

## Development Commands

### Rust Backend
```bash
# Build all Rust crates
cargo build

# Run server (requires database)
cargo run --bin fbkl-server

# Run tests
cargo test

# Lint and format
cargo clippy
cargo fmt

# Database migrations
cd migration && cargo run
```

### Frontend
```bash
# Install dependencies
pnpm install

# Run logged-in webapp
pnpm --filter "@fbkl/webapp-logged-in" dev

# Run public webapp
pnpm --filter "@fbkl/webapp-public" dev

# Lint frontend (oxlint)
pnpm --filter "@fbkl/webapp-logged-in" lint
pnpm --filter "@fbkl/webapp-public" lint

# Format frontend (oxfmt)
pnpm --filter "@fbkl/webapp-logged-in" format
pnpm --filter "@fbkl/webapp-public" format

# TypeScript checking
pnpm --filter "@fbkl/webapp-logged-in" exec tsc
pnpm --filter "@fbkl/webapp-public" exec tsc

# Generate GraphQL types
pnpm --filter "@fbkl/webapp-logged-in" graphql
```

### Git Hooks
Uses Lefthook for pre-commit hooks that automatically run:
- `cargo clippy` and `cargo fmt` on Rust files
- oxlint, oxfmt, and TypeScript checking on frontend files

### Deployment (serverless)
Production runs on AWS Lambda (native Rust) + Supabase Postgres + Cloudflare Pages,
provisioned with OpenTofu in `infra/`. The `lambdas/` crate holds the three
Lambda binaries (`fbkl-api`, `fbkl-scheduler`, `fbkl-session-gc`); `server/`
exposes reusable builders so the local dev bin and the API Lambda share one code
path. See `infra/README.md` for the full setup, required tooling, and apply
order. Required toolchain beyond the dev stack: **OpenTofu** (`tofu`), **AWS CLI
v2** (SSO profile `fbkl`), and **cargo-lambda** (builds the arm64 Lambda zips).
Build artifacts with `cargo lambda build --release --arm64 --output-format zip`
before any `tofu apply` that touches the functions.

## Key Concepts

### End-of-Season Year
Uses basketball season years (e.g., 2017-2018 season = end_of_season_year 2018) rather than calendar years for consistency.

### Transactions
All league actions (trades, drafts, signings, ownership changes) are recorded as transactions for league state reconstruction.

### Business Logic Separation
The `server/` crate contains no business logic - it only handles GraphQL schema and authentication. All fantasy basketball logic lives in `logic/` crate.

### Database Architecture
Uses SeaORM for type-safe database queries. Entity definitions in `entity/` crate with corresponding query functions.

## Code Style and Conventions

### Comments
- Inline/in-body comments: keep concise, ideally one line. If one needs more, simplify the code instead of writing a verbose block.
- File/module header (doc) comments: detailed multi-line blocks are fine and encouraged for explaining a file's purpose and context.

### Rust
- rustfmt with custom config (`rustfmt.toml`): `use_field_init_shorthand = true`, `use_try_shorthand = true`
- Error handling: custom `FbklError` enum (`server/src/error.rs`) with `From` implementations
- Naming: `snake_case` functions/variables, `PascalCase` types

### GraphQL
- Domain-based organization (user, team, league, player, contract)
- Each domain module has separate `types.rs` and `resolvers.rs`
- Root `QueryRoot` and `MutationRoot` in `server/src/graphql.rs`
- Types `PascalCase`, fields `camelCase`

### TypeScript/React
- oxfmt for formatting, oxlint for linting
- New UI work uses shadcn/ui (Base UI) + Tailwind; avoid adding new MUI usage — it is being phased out
- Naming: `camelCase` variables/functions, `PascalCase` components/types

## Task Completion Checklist

Before committing (Lefthook runs clippy/fmt + oxlint/oxfmt/tsc automatically, but run manually to catch early):

**Rust changes**: `cargo build` → `cargo test` → `cargo clippy` → `cargo fmt`. If schema changed, run migrations.

**Frontend changes**: `pnpm --filter <app> lint` → `exec tsc`. If GraphQL schema changed, regenerate types with `pnpm --filter "@fbkl/webapp-logged-in" graphql`.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:7510c1e2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
