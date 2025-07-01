# Codebase Structure

## Root Level Organization
```
fbkl-rust/
├── Backend Rust Crates/
│   ├── server/          # GraphQL API server (no business logic)
│   ├── logic/           # Core fantasy basketball business logic
│   ├── entity/          # Database models and queries (SeaORM)
│   ├── auth/            # Authentication and authorization
│   ├── migration/       # Database schema migrations
│   ├── constants/       # Shared constants
│   ├── jobs/            # Background job processing
│   ├── transaction-processor/  # League transaction processing
│   ├── import-data/     # ESPN NBA data import utilities
│   └── graphql-generation/     # GraphQL schema generation
├── Frontend Apps/
│   ├── webapp-logged-in/    # Next.js app for authenticated users
│   └── webapp-public/       # Vite app for public users
├── Configuration/
│   ├── notes/               # Design documents and specifications
│   ├── Cargo.toml          # Rust workspace configuration
│   ├── package.json        # Node.js workspace configuration
│   ├── lefthook.yml        # Git hooks configuration
│   ├── rustfmt.toml        # Rust formatting configuration
│   └── .prettierrc.json    # TypeScript/React formatting
```

## Key Architectural Principles
1. **Clear Separation of Concerns**: Server crate only handles GraphQL schema and auth
2. **Business Logic Isolation**: All fantasy basketball logic in dedicated logic crate
3. **Type Safety**: SeaORM for database, TypeScript for frontend
4. **Transaction-Driven**: All league actions recorded as transactions
5. **Modular Design**: Each crate has single responsibility

## GraphQL API Structure
- Domain-based organization (user, team, league, player, contract)
- Separate types.rs and resolvers.rs files
- Root QueryRoot and MutationRoot in server/src/graphql.rs

## Database Layer
- Entity definitions in entity/ crate
- Migration management in migration/ crate
- Type-safe queries with SeaORM throughout