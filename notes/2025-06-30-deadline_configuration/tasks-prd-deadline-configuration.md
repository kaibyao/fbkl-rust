# Tasks: Deadline Configuration System Implementation

Based on the PRD for Deadline Configuration System, focusing on Phase 1 (Core Configuration) and Phase 2 (Processing Engine & Source Attribution).

## Relevant Files

- `entity/src/entities/deadline_config_rule.rs` - Already exists - deadline configuration entity with database model
- `entity/src/entities/deadline.rs` - Already exists - enhanced with status field and DeadlineStatus enum
- `entity/src/queries/deadline_config_rule_queries.rs` - Database query functions for configuration rules
- `logic/src/deadline_config/mod.rs` - Exists but empty - core business logic for deadline configuration
- `logic/src/deadline_config/validation.rs` - Validation logic for deadline configuration rules
- `logic/src/deadline_config/generation.rs` - Logic to generate deadline records from configuration rules
- `logic/src/deadline_processing/mod.rs` - Deadline processing engine with deadline generation and processing functions
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

- [x] 2.0 **Create GraphQL API Layer**
  - [x] 2.1 Define GraphQL input/output types in `server/src/graphql/deadline_config/types.rs`
  - [x] 2.2 Implement configuration mutations (configure, activate) in `server/src/graphql/deadline_config/resolvers.rs`
  - [x] 2.3 Implement configuration queries (get config, get status) in `server/src/graphql/deadline_config/resolvers.rs`
  - [x] 2.4 Add authorization checks for commissioner-only access
  - [x] 2.5 Integrate deadline config module with main GraphQL schema

- [ ] 3.0 **Build Deadline Processing Engine**
  - [x] 3.1 Implement deadline generation from activated config rules in `logic/src/deadline_processing/mod.rs`
  - [x] 3.2 Create idempotent deadline processing functions with dependency handling
  - [x] 3.3 Add deadline status transition logic (Draft → Activated → Processing → Processed)
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

## Phase 3 Implementation Plan: Deadline Processing Engine

### Current State Analysis

Based on codebase analysis, **Phases 1 & 2 are mostly complete**:

- ✅ **Configuration Storage**: `deadline_config_rule` entity and queries implemented
- ✅ **Validation**: Comprehensive validation logic in `logic/src/deadline_config/validation.rs`
- ✅ **Generation**: Deadline generation from config rules in `logic/src/deadline_config/generation.rs`
- ✅ **GraphQL API**: Full API with commissioner authorization in `server/src/graphql/deadline_config/`
- ✅ **Status Management**: `DeadlineStatus` enum (Draft, Activated, Processing, Processed, Error)
- ⚠️  **Jobs Infrastructure**: `jobs/src/lib.rs` exists but is mostly empty
- ⚠️  **Processing Engine**: `logic/src/deadline_processing/mod.rs` has basic structure but needs expansion

### Detailed Task Breakdown

#### Task 3.1: Generate Deadlines from Activated Config Rules
**File**: `logic/src/deadline_processing/mod.rs`

**Implementation Approach**:
- Create `generate_deadlines_if_needed()` function that:
  - Finds leagues with deadline configs but no corresponding deadline records
  - Uses existing `generate_deadlines_from_config()` from `generation.rs`
  - Creates deadline records with `Activated` status (not Draft)
  - Handles config updates by cleaning up/updating existing deadlines
- Leverage existing query functions from `deadline_config_rule_queries.rs`
- Use database transactions for atomicity

**Key Functions to Create**:
```rust
pub async fn generate_deadlines_if_needed<C>(db: &C) -> Result<Vec<(i64, i16, Vec<deadline::Model>)>>
pub async fn cleanup_outdated_deadlines<C>(league_id: i64, end_of_season_year: i16, db: &C) -> Result<()>
```

**Estimated Time**: 2-3 hours

#### Task 3.2: Idempotent Processing with Dependency Handling
**File**: `logic/src/deadline_processing/mod.rs`

**Implementation Approach**:
- Create `process_activated_deadlines()` function that:
  - Finds deadlines with `Activated` status where current time >= deadline time
  - Processes in dependency order using existing `get_deadline_dependency_order()`
  - Leverages existing deadline processing logic in `keeper_deadline/` and `roster_lock/`
  - Implements atomic status transitions (Activated → Processing → Processed/Error)
  - Uses database locking for concurrency safety
- Design functions to accept optional `source_deadline_id` for future attribution

**Key Functions to Create**:
```rust
pub async fn process_activated_deadlines<C>(db: &C) -> Result<Vec<ProcessingResult>>
pub async fn process_single_deadline<C>(deadline: &deadline::Model, db: &C) -> Result<ProcessingResult>
pub async fn check_deadline_prerequisites<C>(deadline: &deadline::Model, db: &C) -> Result<bool>
```

**Integration Points**:
- `keeper_deadline::process_keeper_deadline()` - existing function
- `roster_lock::lock_rosters()` - existing function
- Extend these to accept `source_deadline_id` parameter

**Estimated Time**: 4-5 hours

#### Task 3.3: Status Transition Logic
**File**: `logic/src/deadline_processing/mod.rs`

**Implementation Approach**:
- Create `transition_deadline_status()` helper function
- Implement state machine with proper error handling:
  - Draft → Activated (manual via GraphQL)
  - Activated → Processing (automatic, via cron)
  - Processing → Processed (on success)
  - Processing → Error (on failure, with error details)
- Add comprehensive audit logging for all transitions
- Use database transactions to ensure atomicity

**Key Functions to Create**:
```rust
pub async fn transition_deadline_status<C>(deadline_id: i64, new_status: DeadlineStatus, error_message: Option<String>, db: &C) -> Result<()>
pub async fn rollback_processing_status<C>(deadline_id: i64, error: &str, db: &C) -> Result<()>
```

**Estimated Time**: 1-2 hours

#### Task 3.4: Cron Job Implementation
**File**: `jobs/src/deadline_processor.rs` (new file)

**Implementation Approach**:
- Create main `process_deadlines()` function for cron execution
- Implement concurrency safety with `SELECT ... FOR UPDATE SKIP LOCKED`
- Call `generate_deadlines_if_needed()` and `process_activated_deadlines()`
- Add comprehensive error handling and logging
- Make function idempotent and safe for concurrent execution
- Update `jobs/src/lib.rs` to export the new module

**Key Functions to Create**:
```rust
pub async fn process_deadlines() -> Result<ProcessingReport>
pub async fn acquire_processing_lock<C>(db: &C) -> Result<Option<ProcessingLock>>
pub async fn log_processing_results(results: &ProcessingReport) -> Result<()>
```

**Infrastructure Setup**:
- Add proper logging configuration
- Add database connection management
- Design for horizontal scalability

**Estimated Time**: 2-3 hours

#### Task 3.5: Admin Manual Processing Mutation
**File**: `server/src/graphql/deadline_config/resolvers.rs`

**Implementation Approach**:
- Add `process_deadline_now()` mutation to `DeadlineConfigMutation`
- Extend existing `check_commissioner_access()` to support admin role checking
- Allow manual triggering of specific deadline processing
- Add comprehensive error handling and audit logging
- Support both single deadline and bulk processing

**Key Functions to Add**:
```rust
async fn process_deadline_now(&self, ctx: &Context<'_>, deadline_id: i64) -> Result<bool>
async fn process_all_ready_deadlines(&self, ctx: &Context<'_>, league_id: i64, end_of_season_year: i16) -> Result<ProcessingReport>
async fn check_admin_access(&self, ctx: &Context<'_>) -> Result<bool>
```

**GraphQL Types to Add**:
```rust
struct ProcessingReport {
    processed_count: i32,
    error_count: i32,
    details: Vec<ProcessingDetail>,
}
```

**Estimated Time**: 1-2 hours

### Technical Implementation Strategy

#### 1. Leverage Existing Infrastructure
- **Reuse existing deadline processing logic** in `keeper_deadline/` and `roster_lock/` modules
- **Build on existing status transitions** using the `DeadlineStatus` enum
- **Use existing authorization patterns** from the GraphQL resolvers
- **Follow existing SeaORM patterns** for database operations

#### 2. Idempotent Processing Design
- **Status-based processing**: Only process deadlines in `Activated` status
- **Time-based filtering**: Only process deadlines where current time >= deadline time
- **Database locking**: Use `SELECT ... FOR UPDATE SKIP LOCKED` to prevent race conditions
- **Error recovery**: Implement proper rollback from `Processing` to `Error` status

#### 3. Dependency Handling
- **Sequential processing**: Process deadlines one at a time in dependency order
- **Prerequisite checking**: Ensure previous deadline is `Processed` before starting next
- **Failure isolation**: If one deadline fails, log error but continue with others

#### 4. Concurrency and Safety
- **Database transactions**: Wrap all processing in transactions
- **Lock-based coordination**: Use database locks to coordinate between multiple cron instances  
- **Idempotent operations**: Ensure functions can be safely retried
- **Comprehensive logging**: Track all processing attempts and outcomes

### Integration with Existing Code

#### Database Layer
- Use existing `deadline_queries.rs` functions
- Extend `deadline_config_rule_queries.rs` as needed
- Follow existing SeaORM transaction patterns

#### Business Logic Layer
- Integrate with existing `deadline_config/` modules
- Extend existing deadline processing in `keeper_deadline/` and `roster_lock/`
- Follow existing error handling patterns

#### API Layer
- Use existing authorization and session management
- Follow existing GraphQL resolver patterns
- Maintain consistency with existing mutation/query structure

### Success Criteria

1. **Automated Processing**: Deadlines are automatically processed based on configuration
2. **Dependency Respect**: Deadlines process in correct order with prerequisite checking
3. **Idempotent Operations**: System handles concurrent execution and retries safely
4. **Admin Override**: Administrators can manually trigger processing when needed
5. **Comprehensive Logging**: All processing attempts and outcomes are properly logged
6. **Error Recovery**: Failed processing transitions to Error status with details

### Future Considerations (Phase 4 Preparation)

This implementation will prepare for Phase 4 (Source Attribution) by:
- Designing processing functions to accept optional `source_deadline_id` parameters
- Planning integration points for auction and transaction creation
- Establishing patterns for data attribution and rollback preparation

**Total Estimated Development Time: 10-15 hours across multiple focused sessions**
