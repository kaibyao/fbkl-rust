# Code Style and Conventions

## Rust Style
- **Edition**: 2021
- **Formatting**: rustfmt with custom configuration:
  - `use_field_init_shorthand = true`
  - `use_try_shorthand = true`
- **Linting**: cargo clippy enforced via git hooks
- **Error Handling**: Custom `FbklError` enum with From implementations
- **Module Organization**: Clear separation of concerns with crates

## TypeScript/React Style
- **Formatting**: Prettier with `singleQuote: true`
- **Linting**: ESLint with TypeScript plugin and Next.js rules
- **Import Organization**: 
  - eslint-plugin-sort-imports-es6-autofix
  - eslint-plugin-unused-imports
  - eslint-import-resolver-typescript
- **MUI Imports**: eslint-plugin-mui-path-imports for tree shaking

## Architecture Patterns
- **Business Logic Separation**: Server crate contains NO business logic, delegates to logic crate
- **GraphQL Organization**: Separate modules for each domain (user, team, league, player, contract)
- **Database**: SeaORM entities with type-safe queries
- **Authentication**: Separate auth crate for modularity

## Naming Conventions
- **Rust**: snake_case for functions/variables, PascalCase for types
- **TypeScript**: camelCase for variables/functions, PascalCase for components/types
- **GraphQL**: PascalCase for types, camelCase for fields
- **End-of-season Year**: Use basketball season end year (2017-2018 = 2018)

## File Organization
- **Rust**: src/lib.rs with module declarations, separate files for implementations
- **GraphQL**: types.rs and resolvers.rs separation within domain modules
- **Frontend**: Next.js app directory structure