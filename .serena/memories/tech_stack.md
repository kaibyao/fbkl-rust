# Tech Stack

## Backend (Rust)
- **Language**: Rust (Edition 2021)
- **Database ORM**: SeaORM for type-safe queries
- **API**: GraphQL with async-graphql
- **Authentication**: Custom auth with Argon2 for password hashing
- **Web Framework**: Axum
- **Package Manager**: Cargo

## Frontend (TypeScript/React)
- **Language**: TypeScript
- **Framework**: Next.js (logged-in app) and Vite (public app)
- **UI Library**: Material-UI (MUI) v7
- **GraphQL Client**: urql
- **Forms**: react-hook-form
- **Package Manager**: pnpm v10.11.1

## Development Tools
- **Linting**: ESLint with TypeScript plugin
- **Formatting**: Prettier (singleQuote: true) + rustfmt
- **Git Hooks**: Lefthook for pre-commit automation
- **Type Generation**: GraphQL Code Generator
- **Monorepo**: Cargo workspace + pnpm workspace

## Database
- **Migrations**: SeaORM CLI migrations

## Deployment Ports
- **Logged-in webapp**: Port 9100
- **Server**: Standard GraphQL endpoint