# Task Completion Checklist

## Before Committing Code

### Rust Backend Changes
```bash
# 1. Build and check for compilation errors
cargo build

# 2. Run clippy for linting
cargo clippy

# 3. Format code
cargo fmt

# 4. If database changes, run migrations
cd migration && cargo run
```

### Frontend Changes
```bash
# 1. Lint the code
pnpm --filter "@fbkl/webapp-logged-in" lint
pnpm --filter "@fbkl/webapp-public" lint

# 2. Type check
pnpm --filter "@fbkl/webapp-logged-in" exec tsc
pnpm --filter "@fbkl/webapp-public" exec tsc

# 3. If GraphQL schema changed, regenerate types
pnpm --filter "@fbkl/webapp-logged-in" graphql
```

## Automated Pre-commit Checks
Lefthook will automatically run these on commit:
- cargo clippy + cargo fmt for Rust files
- ESLint + TypeScript checking for frontend files

## Testing
- No specific test commands found in configuration
- Verify functionality manually or ask user for test procedures

## Integration Checks
- Ensure server starts successfully: `cargo run --bin fbkl-server`
- Verify frontend builds: check dev server startup
- Database connectivity if changes affect data layer

## Code Quality Standards
- Follow business logic separation (no logic in server crate)
- Maintain type safety with SeaORM and TypeScript
- Use proper error handling with FbklError
- Follow established naming and module organization patterns