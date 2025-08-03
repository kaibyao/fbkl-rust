use async_graphql::{InputObject, Object, SimpleObject};
use fbkl_entity::{deadline, deadline_config_rule};
use fbkl_logic::deadline_config::ValidationError;

/// GraphQL input type for configuring deadline rules
#[derive(InputObject)]
pub struct DeadlineConfigRulesInput {
    /// Absolute datetime for preseason keeper deadline (ISO 8601 format)
    pub preseason_keeper_deadline: String,
    /// Days after keeper deadline when veteran auction starts (at 9am)
    pub veteran_auction_days_after_keeper_deadline_duration: i16,
    /// Number of days free agent auction remains open
    pub fa_auction_days_duration: i16,
    /// Days after rookie draft ends when final roster lock occurs
    pub final_roster_lock_deadline_days_after_rookie_draft: i16,
    /// Week number when playoffs start (default 21)
    pub playoffs_start_week: i16,
}

/// GraphQL output type for deadline configuration rules
#[derive(Clone, Default, SimpleObject)]
pub struct DeadlineConfigRules {
    pub id: i64,
    pub league_id: i64,
    pub end_of_season_year: i16,
    pub preseason_keeper_deadline: String,
    pub veteran_auction_days_after_keeper_deadline_duration: i16,
    pub fa_auction_days_duration: i16,
    pub final_roster_lock_deadline_days_after_rookie_draft: i16,
    pub playoffs_start_week: i16,
}

impl DeadlineConfigRules {
    pub fn from_model(model: deadline_config_rule::Model) -> Self {
        Self {
            id: model.id,
            league_id: model.league_id,
            end_of_season_year: model.end_of_season_year,
            preseason_keeper_deadline: model.preseason_keeper_deadline.to_rfc3339(),
            veteran_auction_days_after_keeper_deadline_duration: model
                .veteran_auction_days_after_keeper_deadline_duration,
            fa_auction_days_duration: model.fa_auction_days_duration,
            final_roster_lock_deadline_days_after_rookie_draft: model
                .final_roster_lock_deadline_days_after_rookie_draft,
            playoffs_start_week: model.playoffs_start_week,
        }
    }
}

/// Status information for deadline configuration
#[derive(Clone, Default, SimpleObject)]
pub struct DeadlineConfigStatus {
    /// Whether configuration exists for this league/season
    pub has_configuration: bool,
    /// Whether deadlines have been activated
    pub is_activated: bool,
    /// Number of deadlines in Draft status
    pub draft_deadlines_count: i32,
    /// Number of deadlines in Activated status
    pub activated_deadlines_count: i32,
    /// Number of deadlines that have been processed
    pub processed_deadlines_count: i32,
    /// Whether commissioner can still edit the configuration
    pub can_edit: bool,
}

/// Validation error for deadline configuration
#[derive(Clone, SimpleObject)]
pub struct DeadlineConfigValidationError {
    pub field: String,
    pub message: String,
}

impl From<ValidationError> for DeadlineConfigValidationError {
    fn from(error: ValidationError) -> Self {
        Self {
            field: error.field,
            message: error.message,
        }
    }
}

/// Result of deadline configuration validation
#[derive(Clone, SimpleObject)]
pub struct DeadlineConfigValidationResult {
    pub is_valid: bool,
    pub errors: Vec<DeadlineConfigValidationError>,
}

impl From<fbkl_logic::deadline_config::ValidationResult> for DeadlineConfigValidationResult {
    fn from(result: fbkl_logic::deadline_config::ValidationResult) -> Self {
        Self {
            is_valid: result.is_valid,
            errors: result.errors.into_iter().map(|e| e.into()).collect(),
        }
    }
}

/// Enhanced Deadline type that includes status field
#[derive(Clone)]
pub struct Deadline {
    pub id: i64,
    pub date_time: String,
    pub kind: deadline::DeadlineKind,
    pub name: String,
    pub end_of_season_year: i16,
    pub league_id: i64,
    pub status: deadline::DeadlineStatus,
}

impl Deadline {
    pub fn from_model(model: deadline::Model) -> Self {
        Self {
            id: model.id,
            date_time: model.date_time.to_rfc3339(),
            kind: model.kind,
            name: model.name,
            end_of_season_year: model.end_of_season_year,
            league_id: model.league_id,
            status: model.status,
        }
    }
}

#[Object]
impl Deadline {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn date_time(&self) -> String {
        self.date_time.clone()
    }

    async fn kind(&self) -> deadline::DeadlineKind {
        self.kind
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn end_of_season_year(&self) -> i16 {
        self.end_of_season_year
    }

    async fn league_id(&self) -> i64 {
        self.league_id
    }

    async fn status(&self) -> deadline::DeadlineStatus {
        self.status
    }
}
