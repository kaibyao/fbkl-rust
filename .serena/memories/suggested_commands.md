# Suggested Commands

## Rust Backend Development

### Build and Run
```bash
# Build all Rust crates
cargo build

# Run the GraphQL server (requires database)
cargo run --bin fbkl-server

# Database migrations
cd migration && cargo run
```

### Code Quality
```bash
# Lint Rust code
cargo clippy

# Format Rust code
cargo fmt
```

## Frontend Development

### Dependencies
```bash
# Install all dependencies
pnpm install
```

### Development Servers
```bash
# Run logged-in webapp (port 9100)
pnpm --filter "@fbkl/webapp-logged-in" dev

# Run public webapp
pnpm --filter "@fbkl/webapp-public" dev
```

### Code Quality
```bash
# Lint frontend code
pnpm --filter "@fbkl/webapp-logged-in" lint
pnpm --filter "@fbkl/webapp-public" lint

# TypeScript type checking
pnpm --filter "@fbkl/webapp-logged-in" exec tsc
pnpm --filter "@fbkl/webapp-public" exec tsc

# Generate GraphQL types
pnpm --filter "@fbkl/webapp-logged-in" graphql
```

## System Commands (Darwin)
- `ls` - List files
- `find` - Search for files
- `grep` - Search within files
- `git` - Version control
- `cd` - Change directory

## Automated Hooks
Lefthook automatically runs on git commits:
- `cargo clippy` and `cargo fmt` for Rust files
- ESLint and TypeScript checking for frontend files