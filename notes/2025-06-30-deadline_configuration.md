# Deadline Configuration

A league commissioner user needs to be able to set the deadlines for the next end-of-season year after the current season ends.

Up until the point that the commissioner "activates" the season (which means we probably need a status column in the deadline table or a new table for storing league-season configs), they should have the ability to update deadline dates as much as they want, and deadlines should not be processed if they aren't activated.

After activation, deadlines that haven't been processed yet (because they are in the future) should still be editable.

As for the actual deadlines that need to be set:
1. PreseasonKeeper - Datetime.
2. PreseasonVeteranAuctionStart - (Integer) Days after preseason keeper, at 9am.
3. PreseasonFaAuctionStart - (Integer) days to remain up, starts automatically 3 hours after the last veteran auction has started.
4. Preseason Final roster lock - (Integer) days after the rookie draft has ended.
5. Playoffs start at week # - (Integer choice, default 21).

The rest don't need to be explicitly set because those phases of the pre-season/season just happen automatically after the previous stage has finished:
1. PreseasonRookieDraftStart can start automatically 3 days after the last veteran auction has started, at 9am.
2. Week1FreeAgentAuctionStart - Starts automatically when preseason final roster lock has been processed. Lasts until Week1RosterLock.

... But what if a commissioner needs to change a deadline date after the deadline has already been processed because it was in the past? In those cases, I think we almost need a serial "rollback" strategy...
1. Delete any auctions / transactions / contracts created as part of the deadlines being processed, from most recent deadline back to the target deadline that needs to be edited.
2. Deactivate the deadline being edited as well as subsequent deadlines (This makes me think the status field needs to be on the deadlines table. This also makes me think we might want a `processed` status so we can do rollbacks on just the deadlines that have been processed).

I think we also need to hardcode the in-season roster lock deadlines in the codebase.

## Implementation Plan

### Architecture Overview

```
System Components & Data Flow:
┌─────────────────────────────────────────────────────────────────────────────┐
│                           FBKL Deadline Configuration System                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Frontend (webapp-logged-in)                 Backend (Rust)                 │
│  ┌─────────────────────────┐                 ┌─────────────────────────────┐ │
│  │ Commissioner Dashboard  │ <-- GraphQL --> │ server/ (GraphQL API)       │ │
│  │ - Deadline Config Form  │                 │ - deadline_config resolvers │ │
│  │ - Status Display        │                 │ - Authorization checks      │ │
│  │ - Edit Configuration    │                 └─────────────────────────────┘ │
│  └─────────────────────────┘                               │               │
│                                                            │               │
│  ┌─────────────────────────┐                 ┌─────────────────────────────┐ │
│  │ Admin Panel             │ <-- GraphQL --> │ logic/ (Business Logic)     │ │
│  │ - Rollback Interface    │                 │ - Deadline validation       │ │
│  │ - System Monitoring     │                 │ - Versioning logic          │ │
│  │ - Manual Processing     │                 │ - Safe rollback functions   │ │
│  └─────────────────────────┘                 └─────────────────────────────┘ │
│                                                            │               │
│                                               ┌─────────────────────────────┐ │
│                                               │ entity/ (Database Layer)    │ │
│                                               │ - LeagueDeadlineConfig      │ │
│                                               │ - Deadline-specific tables  │ │
│                                               │ - SeaORM queries            │ │
│                                               └─────────────────────────────┘ │
│                                                            │               │
│  ┌─────────────────────────┐                 ┌─────────────────────────────┐ │
│  │ Cron Jobs               │                 │ jobs/ (Processing Engine)   │ │
│  │ - Every 1 minute        │ <-- Triggers -> │ - Idempotent processors     │ │
│  │ - Process deadlines     │                 │ - Dependency checking       │ │
│  │ - Error handling        │                 │ - Source attribution        │ │
│  └─────────────────────────┘                 └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Database Schema Design

**Existing Schema Enhancement:**
The system already has a `deadline` table. We'll enhance it with configuration capabilities:

```sql
-- Add columns to existing deadline table via migration
-- Draft, Active, Processed
ALTER TABLE deadline ADD COLUMN status TEXT DEFAULT 'Processed';

-- New table for deadline configuration rules
CREATE TABLE deadline_config_rules (
  id BIGSERIAL PRIMARY KEY,
  league_id BIGINT NOT NULL REFERENCES league(id),
  end_of_season_year SMALLINT NOT NULL,

  -- Configuration values for each deadline type
  preseason_keeper_datetime TIMESTAMPTZ NOT NULL,
  veteran_auction_days_after_keeper SMALLINT NOT NULL,
  fa_auction_days_duration SMALLINT NOT NULL,
  final_roster_lock_days_after_rookie_draft SMALLINT NOT NULL,
  playoffs_start_week SMALLINT NOT NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- One config per league/season
  CONSTRAINT unique_config UNIQUE (league_id, end_of_season_year)
);

-- Add source attribution to existing tables
ALTER TABLE auction ADD COLUMN deadline_id BIGINT REFERENCES deadline(id);
```

### Phase-Based Implementation Strategy

## Phase 1: Core Configuration System

**Objective:** Enable commissioners to configure and save deadline sets

### Step 1.1: Database Schema Implementation
- Create migration to enhance existing `deadline` table:
  - Add `status` column (TEXT DEFAULT 'Processed')
- Create new `deadline_config_rules` table:
  - id (BIGSERIAL primary key)
  - league_id (BIGINT, references league)
  - end_of_season_year (SMALLINT)
  - Configuration fields:
    - preseason_keeper_datetime (TIMESTAMPTZ)
    - veteran_auction_days_after_keeper (INT)
    - fa_auction_days_duration (INT)
    - final_roster_lock_days_after_rookie_draft (INT)
    - playoffs_start_week (INT DEFAULT 21)
  - Unique constraint on (league_id, end_of_season_year)

### Step 1.2: Entity Crate Implementation
- Enhance existing `deadline` entity:
  - Add new field: status
  - Add DeadlineStatus enum (Draft, Activated, Processing, Processed, Error)
- Create new `deadline_config_rule` entity:
  - Define struct with all configuration fields
  - Add relationships to league entity
  - Add query functions:
    - get_config_rules() - fetch current configuration
    - upsert_config_rules() - insert or update configuration

### Step 1.3: Business Logic Layer (logic/ crate)
- Create deadline_config module with core business functions:
  - configure_deadlines() - main function to save configuration rules
  - generate_deadlines_from_rules() - creates actual deadline records from rules
  - validate_config_rules() - ensures dates and offsets make sense
  - can_edit_deadlines() - checks if deadlines are still in Draft status
  - calculate_absolute_dates() - converts relative offsets to absolute DateTimes
- Integration with existing deadline logic:
  - Update existing deadline creation to use config rules
  - Maintain backward compatibility with hardcoded deadlines
- Add comprehensive error handling with FbklError variants

### Step 1.4: GraphQL API Implementation (server/ crate)
- Create deadline_config module in server/src/graphql/:
  - deadline_config_types.rs with GraphQL types:
    - DeadlineConfigRulesInput (single input with all 5 deadline config values)
    - DeadlineConfigRules (output type matching database structure)
    - Enhanced Deadline type to include new status field
  - deadline_config_resolvers.rs with mutations and queries:
    - configureSeasonDeadlines mutation - saves config rules
    - activateSeasonDeadlines mutation - changes status to Activated
    - getDeadlineConfigRules query - fetch configuration
    - getDeadlines query - enhanced to show status
- Integrate with existing GraphQL schema structure
- Add authorization checks (only league commissioners can configure)

### Step 1.5: Frontend Type Generation
- Update graphql-generation/ crate to include new types
- Generate TypeScript types for webapp-logged-in
- Ensure type safety for deadline configuration forms

## Phase 2: Processing Engine & Source Attribution

**Objective:** Automatic deadline processing with proper dependency handling

### Step 2.1: Add Source Attribution to Existing Systems
- Add source_deadline_id column to core tables:
  - auction table
  - Any other tables that deadlines create
- Update existing business logic in logic/ crate to accept and propagate source_deadline_id
- Ensure all transaction creation functions can track their source deadline

### Step 2.2: Deadline Processing Engine (jobs/ crate)
- Create deadline_processor module with:
  - process_deadlines() main function for cron job
  - generate_deadlines_if_configured() - creates deadline records from config rules
  - process_activated_deadlines() - processes deadlines in Activated status
  - Leverage existing deadline processing logic for each DeadlineKind
- Implement dependency checking based on DeadlineKind order
- Use SELECT ... FOR UPDATE SKIP LOCKED for concurrency safety
- Integration with existing deadline processing infrastructure

### Step 2.3: Idempotent Job Implementation
- Implement robust error handling and status transitions:
  - Draft -> Activated (manual)
  - Activated -> Processing (automatic, via cron)
  - Processing -> Processed (on success)
  - Processing -> Error (on failure, with error details)
- Add retry logic for transient failures
- Implement deadline calculation logic (absolute times from relative rules)
- Ensure jobs are idempotent and can handle partial failures gracefully

### Step 2.4: Manual Processing Trigger
- Add GraphQL mutation activateSeasonDeadlines for commissioners
- Add GraphQL mutation processDeadlineNow for admin emergency processing
- Include authorization checks (commissioners for activate, admins for manual process)

## Phase 3: Admin Rollback Tooling

**Objective:** Safe rollback capabilities for processed deadlines

### Step 3.1: Rollback Flag Infrastructure
- Add is_rolled_back columns to affected tables:
  - transactions.is_rolled_back (BOOLEAN DEFAULT FALSE)
  - auctions.is_rolled_back (BOOLEAN DEFAULT FALSE)
  - Add indexes for efficient querying of non-rolled-back records

### Step 3.2: Admin Rollback Logic
- Implement safe rollback functions in logic/ crate:
  - find_deadline_artifacts() - query all entities created by a deadline
  - flag_artifacts_as_rolled_back() - set is_rolled_back flags
  - rollback_deadline_config() - set deadline status to Draft
- Add extensive logging and audit trail for rollback actions
- Implement rollback validation (prevent rollback of critical processed items)

### Step 3.3: Admin UI/API for Rollback
- Add GraphQL mutation rollbackDeadline (admin-only)
- Add GraphQL query getDeadlineArtifacts for preview before rollback
- Implement comprehensive authorization checks
- Add frontend admin interface for rollback operations

### Step 3.4: Application Logic Updates
- Update all queries to filter out rolled-back records by default
- Add explicit include_rolled_back parameters where needed for admin views
- Update business logic to ignore rolled-back transactions and auctions
- Ensure consistency across all read operations

## Testing & Integration

### Step 4.1: Unit Testing
- Add comprehensive tests for logic/ crate deadline functions:
  - Test deadline validation and business rules
  - Test configuration update logic
  - Test dependency resolution
  - Test error handling and edge cases
- Add tests for entity/ crate queries and relationships
- Mock database interactions for isolated testing

### Step 4.2: Integration Testing
- Test full GraphQL API endpoints with test database
- Test deadline processing jobs with realistic data
- Test rollback scenarios end-to-end
- Test concurrency scenarios (multiple commissioners, race conditions)
- Test failure recovery and idempotency

### Step 4.3: Frontend Implementation (webapp-logged-in)
- Create deadline configuration page for commissioners:
  - Form with all 5 deadline input fields
  - Validation and user-friendly error messages
  - Status display for current deadline configuration
- Integrate with existing league management UI
- Add commissioner authorization checks in frontend

### Step 4.4: Admin Frontend Implementation
- Create admin panel for deadline management:
  - View all league deadline configurations and statuses
  - Manual deadline processing triggers
  - Rollback interface with artifact preview
  - System status monitoring for deadline jobs
- Add comprehensive error handling and user feedback
- Implement proper admin-only authorization checks

## Deployment & Monitoring

### Step 5.1: Cron Job Setup
- Configure cron job for deadline processing:
  - Schedule: every 5-15 minutes for timely processing
  - Logging: comprehensive logs for monitoring and debugging
  - Error handling: notifications for failed deadline processing
  - Resource monitoring: ensure job doesn't overwhelm system
- Add manual trigger capability for immediate processing

### Step 5.2: Database Migration Planning
- Create comprehensive migration scripts for production deployment
- Plan rollback migrations in case of issues
- Test migrations on production-like data sets
- Coordinate with existing database versioning system

### Step 5.3: Monitoring & Observability
- Add deadline-specific logging and metrics:
  - Track deadline processing success/failure rates
  - Monitor processing latency and performance
  - Alert on deadline processing failures
  - Dashboard for system health monitoring
- Integrate with existing monitoring infrastructure
- Add comprehensive error reporting for debugging

## Documentation & Validation

### Step 6.1: Technical Documentation
- Update CLAUDE.md with deadline configuration commands and patterns
- Document database schema and relationships
- Create API documentation for GraphQL endpoints
- Document cron job setup and monitoring procedures
- Add troubleshooting guide for common issues

### Step 6.2: User Documentation
- Create commissioner guide for deadline configuration
- Document deadline types and their relationships
- Create admin guide for rollback procedures
- Add FAQ for common deadline configuration scenarios
- Document business rules and validation constraints

### Step 6.3: End-to-End Validation
- Conduct full system testing with realistic league data:
  - Test complete deadline configuration workflow
  - Validate deadline processing with all dependency chains
  - Test rollback scenarios with real auction/transaction data
  - Performance testing under load
  - Security testing for authorization boundaries
- User acceptance testing with league commissioners
- Load testing for cron job processing at scale

## Deployment Readiness

### Step 7.1: Production Deployment Plan
- Create deployment checklist and procedures:
  - Database migration execution plan
  - Cron job configuration steps
  - Monitoring setup verification
  - Rollback procedures if deployment fails
- Plan feature flag approach for gradual rollout
- Coordinate with existing deployment pipeline
- Schedule deployment window and communication plan

## Success Criteria & Risk Mitigation

### Success Criteria for Each Phase:
- **Phase 1:** Commissioners can successfully configure and save deadline rules for their leagues
- **Phase 2:** System generates and processes deadlines based on configuration rules
- **Phase 3:** Admins can safely handle deadline errors and make corrections

### Risk Mitigation Strategies:
- Build on existing deadline infrastructure rather than replacing it
- Start with simple status tracking (Draft/Activated/Processed)
- Use database transactions for all configuration changes
- Maintain backward compatibility with existing deadline processing
- Add comprehensive logging from day one

### Key Architecture Benefits:
- **Simplicity:** Single configuration table with straightforward fields and simple upsert operations
- **Leverages Existing Code:** Uses current deadline entity and processing logic
- **Type Safety:** SeaORM entities provide compile-time guarantees
- **Performance:** No need to filter by is_active flags on every query
- **Flexibility:** Can configure each deadline type independently

### Why This Approach is Better:
1. **Less Complex:** One configuration table instead of 6+ tables, no versioning complexity
2. **Easier Queries:** Simple lookups by (league_id, end_of_season_year) - no filtering needed
3. **Faster Development:** Fewer entities, migrations, and GraphQL types
4. **Maintains Compatibility:** Works with existing deadline processing code
5. **Clear Mental Model:** Configuration rules → Generate deadlines → Process as normal
6. **Better Performance:** Direct queries without complex WHERE clauses

The simplified upsert approach eliminates versioning complexity while still meeting all requirements. By building on the existing deadline system with straightforward configuration storage, we minimize both implementation complexity and runtime overhead.
