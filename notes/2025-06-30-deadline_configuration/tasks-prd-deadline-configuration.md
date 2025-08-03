# Tasks: Deadline Configuration System Implementation

Based on the PRD for Deadline Configuration System, focusing on Phase 1 (Core Configuration) and Phase 2 (Processing Engine & Source Attribution).

## Relevant Files

- `entity/src/entities/deadline_config_rule.rs` - Already exists - deadline configuration entity with database model
- `entity/src/entities/deadline.rs` - Already exists - enhanced with status field and DeadlineStatus enum
- `entity/src/queries/deadline_config_rule_queries.rs` - Database query functions for configuration rules
- `logic/src/deadline_config/mod.rs` - Exists but empty - core business logic for deadline configuration
- `logic/src/deadline_config/validation.rs` - Validation logic for deadline configuration rules
- `logic/src/deadline_config/generation.rs` - Logic to generate deadline records from configuration rules
- `logic/src/deadline_processing/mod.rs` - Deadline processing engine and job functions
- `server/src/graphql/deadline_config/mod.rs` - GraphQL module for deadline configuration
- `server/src/graphql/deadline_config/types.rs` - GraphQL input/output types for deadline configuration
- `server/src/graphql/deadline_config/resolvers.rs` - GraphQL resolvers for mutations and queries
- `jobs/src/deadline_processor.rs` - Cron job implementation for deadline processing
- `migration/src/m*_add_deadline_source_attribution.rs` - Migration to add source_deadline_id to auctions/transactions
- `webapp-logged-in/src/components/DeadlineConfig/DeadlineConfigForm.tsx` - Commissioner deadline configuration form
- `webapp-logged-in/src/components/DeadlineConfig/DeadlineConfigForm.test.tsx` - Unit tests for deadline config form
- `webapp-logged-in/src/pages/league/[leagueId]/deadline-config.tsx` - Deadline configuration page
- `webapp-logged-in/src/graphql/deadline-config.graphql` - GraphQL queries and mutations for frontend

### Notes

- Database entities and migrations already exist partially - deadline_config_rule entity is implemented
- The deadline entity already has the status field and DeadlineStatus enum
- Frontend should use existing authentication patterns and league management UI components
- Use existing SeaORM patterns for database queries and transactions
- Follow existing GraphQL schema patterns in server/src/graphql structure

## Tasks

- [x] 1.0 **Implement Core Business Logic Layer**
  - [x] 1.1 Create deadline configuration validation logic in `logic/src/deadline_config/validation.rs`
  - [x] 1.2 Implement deadline generation from config rules in `logic/src/deadline_config/generation.rs`
  - [x] 1.3 Add configuration management functions (upsert, get, activate) in `logic/src/deadline_config/mod.rs`
  - [x] 1.4 Create entity queries for deadline config rules in `entity/src/queries/deadline_config_rule_queries.rs`
  - [x] 1.5 Add comprehensive error handling with FbklError variants for deadline configuration

- [ ] 2.0 **Create GraphQL API Layer**
  - [ ] 2.1 Define GraphQL input/output types in `server/src/graphql/deadline_config/types.rs`
  - [ ] 2.2 Implement configuration mutations (configure, activate) in `server/src/graphql/deadline_config/resolvers.rs`
  - [ ] 2.3 Implement configuration queries (get config, get status) in `server/src/graphql/deadline_config/resolvers.rs`
  - [ ] 2.4 Add authorization checks for commissioner-only access
  - [ ] 2.5 Integrate deadline config module with main GraphQL schema

- [ ] 3.0 **Build Deadline Processing Engine**
  - [ ] 3.1 Implement deadline generation from activated config rules in `logic/src/deadline_processing/mod.rs`
  - [ ] 3.2 Create idempotent deadline processing functions with dependency handling
  - [ ] 3.3 Add deadline status transition logic (Draft → Activated → Processing → Processed)
  - [ ] 3.4 Implement cron job for automatic deadline processing in `jobs/src/deadline_processor.rs`
  - [ ] 3.5 Add manual processing trigger mutation for administrators

- [ ] 4.0 **Add Source Attribution System**
  - [ ] 4.1 Create migration to add source_deadline_id to auctions and transactions tables
  - [ ] 4.2 Update auction creation logic to accept and store source_deadline_id
  - [ ] 4.3 Update transaction creation logic to accept and store source_deadline_id
  - [ ] 4.4 Modify existing deadline processing to propagate source attribution
  - [ ] 4.5 Add queries to trace deadline-created data for future rollback capabilities

- [ ] 5.0 **Implement Frontend Configuration Interface**
  - [ ] 5.1 Create deadline configuration form component with validation
  - [ ] 5.2 Implement deadline status display and activation controls
  - [ ] 5.3 Create deadline configuration page integrated with league management
  - [ ] 5.4 Add GraphQL queries and mutations for frontend consumption
  - [ ] 5.5 Implement commissioner authorization checks in frontend routing
