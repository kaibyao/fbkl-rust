# FBKL

This "application" (really collection of applications) powers FBKL, a custom fantasy basketball ruleset. The ruleset is meant to simulate how real NBA teams manage their assets (IE player contracts and draft picks), while keeping the game fun in a fantasy league setting.

## Project Structure

This monorepo contains multiple applications and libraries that work together to power the FBKL fantasy basketball league system.

### Rust Backend Components

- **`server/`** - Main GraphQL API server that handles authentication and serves the API. Contains no business logic (delegates to `logic/`)

- **`entity/`** - Database models and query functions using SeaORM. Contains struct representations of database tables and related queries

- **`logic/`** - Core business logic for fantasy basketball operations (trades, auctions, roster management, IR, rookie development, etc.)

- **`migration/`** - Database migration management using SeaORM CLI for schema changes and updates

- **`auth/`** - Authentication and authorization logic (separate from server for modularity)

- **`constants/`** - Shared constants used across the Rust workspace

- **`jobs/`** - Background job processing (currently minimal - just `process_keepers()` function)

- **`transaction-processor/`** - Processes league transactions like trades, auctions, drafts, and roster moves. Maintains transaction history for league state reconstruction

- **`import-data/`** - Data import utilities for pulling real-world NBA data from ESPN

- **`graphql-generation/`** - Generates GraphQL schema files for TypeScript type generation in frontend apps

### Frontend Applications

- **`webapp-logged-in/`** - Next.js React app for authenticated users (league members). Uses Material-UI and Apollo GraphQL client

- **`webapp-public/`** - Vite React app for public/unauthenticated users. Also uses Material-UI and Apollo GraphQL client

### Documentation & Configuration

- **`notes/`** - Design documents and specifications for features like keepers, draft picks, transaction deadlines, and trade actions

- **Root files** - Workspace configuration (Cargo.toml for Rust, package.json for Node.js), linting, formatting, and git hooks

## Common terminology

### End-of-season Year

Rather than use numerical years, we use the concept of a Basketball season to mark the year in which a league is run. This is because the NBA Basketball season usually starts in the Fall and ends in the Spring, and it can become confusing to talk about the year in which actions were taken. For example, if a trade was conducted between two teams in 2017, did that trade apply to the 2016-2017 season, or the 2017-2018 season?

Enter the concept of an `end_of_season_year`. In an example where an NBA season runs between October 2017 – May 2018, the "end of season year" for that season is 2018. The "end of season year" can be defined as the year in which a specific NBA Basketball season ends.

### Transaction

Transactions are recordings of the actions taken in the league that result in 1 or more teams changing in some way. Things like trades, drafting a player, signing a free agent, changing team ownership, etc., can all be considered transactions.
