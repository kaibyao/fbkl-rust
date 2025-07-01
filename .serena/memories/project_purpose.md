# Project Purpose

FBKL (Fantasy Basketball League) is a custom fantasy basketball league system that simulates NBA team management. 

## Key Features
- Player contracts and draft picks management
- Trades, auctions, and roster management
- Injured Reserve (IR) and rookie development
- Real NBA team asset simulation for fantasy leagues
- Transaction history tracking for league state reconstruction

## Architecture Style
- Full-stack monorepo with clear separation of concerns
- Backend: Rust with GraphQL API and SeaORM for database operations
- Frontend: React/Next.js with TypeScript and Material-UI
- Business logic completely separated from API layer (server delegates to logic crate)

## Core Domain Concepts
- **End-of-season Year**: Uses basketball season years (e.g., 2017-2018 season = end_of_season_year 2018)
- **Transactions**: All league actions are recorded for state reconstruction
- **Team Management**: Simulates real NBA team operations in fantasy context