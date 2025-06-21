# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

FBKL is a custom fantasy basketball league system built as a full-stack monorepo. The backend uses Rust with GraphQL/SeaORM, and the frontend uses React/Next.js/TypeScript. The system simulates NBA team management with player contracts, trades, auctions, and draft picks.

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
- **`webapp-logged-in/`** - Next.js app for authenticated users (port 9100)
- **`webapp-public/`** - Vite app for public users

## Development Commands

### Rust Backend
```bash
# Build all Rust crates
cargo build

# Run server (requires database)
cargo run --bin fbkl-server

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

# Lint frontend
pnpm --filter "@fbkl/webapp-logged-in" lint
pnpm --filter "@fbkl/webapp-public" lint

# TypeScript checking
pnpm --filter "@fbkl/webapp-logged-in" exec tsc
pnpm --filter "@fbkl/webapp-public" exec tsc

# Generate GraphQL types
pnpm --filter "@fbkl/webapp-logged-in" graphql
```

### Git Hooks
Uses Lefthook for pre-commit hooks that automatically run:
- `cargo clippy` and `cargo fmt` on Rust files
- ESLint and TypeScript checking on frontend files

## Key Concepts

### End-of-Season Year
Uses basketball season years (e.g., 2017-2018 season = end_of_season_year 2018) rather than calendar years for consistency.

### Transactions
All league actions (trades, drafts, signings, ownership changes) are recorded as transactions for league state reconstruction.

### Business Logic Separation
The `server/` crate contains no business logic - it only handles GraphQL schema and authentication. All fantasy basketball logic lives in `logic/` crate.

### Database Architecture
Uses SeaORM for type-safe database queries. Entity definitions in `entity/` crate with corresponding query functions.