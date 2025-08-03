# Product Requirements Document: Deadline Configuration System

## Introduction/Overview

The FBKL fantasy basketball league system currently requires manual deadline management for each season. This feature will enable league commissioners to configure and automatically process season deadlines through a streamlined interface, reducing manual work and support requests.

**Problem:** Commissioners currently cannot set or modify deadline dates for their leagues, leading to increased support requests and manual intervention requirements.

**Goal:** Provide commissioners with a self-service deadline configuration system that automatically processes league events based on their configured timeline.

## Goals

1. **Reduce Support Burden**: Decrease deadline-related support requests by 75% through self-service configuration
2. **Improve Commissioner Experience**: Enable commissioners to easily configure season deadlines without technical intervention
3. **Increase System Reliability**: Automate deadline processing with proper dependency handling and error recovery
4. **Maintain Data Integrity**: Ensure all deadline-driven events (auctions, roster locks) are properly tracked and attributable

## User Stories

### Primary User Stories
**As a league commissioner**, I want to configure deadline dates for the upcoming season so that my league operates on our preferred timeline.

**As a league commissioner**, I want to modify future deadlines before they're processed so that I can adjust for schedule changes.

**As a league commissioner**, I want to activate my deadline configuration so that the system begins automatic processing.

**As a system administrator**, I want to view all league deadline configurations so that I can monitor system health and assist with issues.

**As a system administrator**, I want to manually trigger deadline processing so that I can resolve system errors or special circumstances.

### Secondary User Stories
**As a league member**, I want to see upcoming deadlines in my league so that I can plan my activities accordingly.

**As a developer**, I want all deadline-created data to be properly attributed so that we can trace system actions and support rollback capabilities.

## Functional Requirements

### Core Configuration (Phase 1)
1. The system must allow commissioners to configure five deadline types for their league:
   - Preseason Keeper deadline (absolute datetime)
   - Veteran Auction Start (days after keeper deadline, at 9am)
   - Free Agent Auction Duration (days to remain open)
   - Final Roster Lock (days after rookie draft ends)
   - Playoffs Start Week (week number, default 21)

2. The system must validate deadline configurations to prevent logical conflicts:
   - Playoff start week must be after roster lock
   - All day offsets must be positive integers
   - Preseason keeper datetime must be in the future

3. The system must store one configuration per league per season with upsert functionality

4. The system must support three deadline statuses: Draft, Activated, and Processed

5. The system must restrict deadline editing based on status:
   - Draft: All deadlines editable
   - Activated: Only future deadlines editable
   - Processed: No editing allowed

### Automatic Processing (Phase 2)
6. The system must automatically generate deadline records from configuration rules when activated

7. The system must process activated deadlines according to dependency order:
   - Preseason Keeper → Veteran Auction Start → FA Auction Start → Rookie Draft Start → Final Roster Lock

8. The system must run deadline processing jobs every 5-15 minutes via cron

9. The system must ensure idempotent processing to handle job failures and retries

10. The system must attribute all deadline-created data (auctions, transactions) to source deadlines

### Authorization & Access Control
11. The system must restrict deadline configuration to league commissioners only

12. The system must allow both commissioners and administrators to manually trigger processing

13. The system must provide read-only access to deadline status for all league members

### API & Integration
14. The system must provide GraphQL mutations for:
    - Configure season deadlines
    - Activate season deadlines  
    - Manual deadline processing (admin)

15. The system must provide GraphQL queries for:
    - Get deadline configuration rules
    - Get deadline status and history

16. The system must integrate with existing authentication and authorization systems

## Non-Goals (Out of Scope)

1. **Rollback Functionality**: Deadline rollback capabilities are excluded from this initial implementation
2. **Historical Versioning**: Configuration change history tracking is not included
3. **Advanced UI Features**: Calendar integration, visual timelines, or multi-step wizards are not included
4. **Custom Deadline Types**: Only the five specified deadline types are supported
5. **Email Notifications**: Automatic deadline status notifications are not included
6. **Trade Deadlines**: Non-auction/draft deadlines are not included in this scope

## Design Considerations

### User Interface
- Simple single-page form with all five deadline configuration fields
- Clear validation messages that prevent saving invalid configurations
- Status indicators showing current deadline state (Draft/Activated/Processed)
- Integration with existing league management interface

### Database Design
- Enhance existing `deadline` table with `status` column
- New `deadline_config_rules` table with configuration values
- Foreign key relationships to maintain data integrity
- Unique constraints to prevent duplicate configurations

### Error Handling
- Comprehensive validation with user-friendly error messages
- Graceful degradation when deadline processing fails
- Detailed logging for debugging and monitoring
- Status transitions that clearly indicate system state

## Technical Considerations

### Dependencies
- Integrates with existing deadline processing logic in `logic/` crate
- Uses SeaORM for type-safe database operations
- Builds on existing GraphQL schema and authentication systems
- Leverages current job processing infrastructure in `jobs/` crate

### Performance
- Configuration queries by (league_id, end_of_season_year) are O(1) with proper indexing
- Deadline processing uses SELECT...FOR UPDATE SKIP LOCKED for concurrency safety
- Minimal impact on existing deadline processing performance

### Scalability
- Single configuration table scales linearly with number of leagues
- Cron job frequency configurable based on system load
- Processing jobs designed to be horizontally scalable

## Success Metrics

**Primary Success Metric**: Reduce deadline-related support requests by 75% within 6 months of deployment

**Secondary Metrics**:
- Commissioner adoption rate: >80% of active leagues using the feature within 3 months
- System reliability: >99% deadline processing success rate
- Configuration accuracy: <5% of configurations requiring modification after activation

**Leading Indicators**:
- Number of leagues with configured deadlines
- Average time between configuration and activation
- Frequency of deadline modifications before activation

## Open Questions

1. Should we implement soft limits on how far in advance deadlines can be configured?
2. Do we need audit logging for configuration changes (who, what, when)?
3. Should commissioners receive confirmation when deadlines are successfully processed?
4. How should we handle timezone considerations for deadline processing?
5. Do we need a "preview mode" to show commissioners what deadlines will be generated?

---

**Implementation Phases**: This PRD covers Phase 1 (Core Configuration System) and Phase 2 (Processing Engine & Source Attribution) from the original implementation plan.

**Target Audience**: Junior developers familiar with Rust, SeaORM, and GraphQL patterns established in the FBKL codebase.