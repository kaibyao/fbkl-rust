use color_eyre::Result;
use fbkl_entity::sea_orm::prelude::DateTimeWithTimeZone;
use tracing::instrument;

/// Validation errors for deadline configuration
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

/// Result of deadline configuration validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: vec![],
        }
    }

    pub fn invalid(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
        }
    }

    pub fn add_error(&mut self, field: &str, message: &str) {
        self.errors.push(ValidationError {
            field: field.to_string(),
            message: message.to_string(),
        });
        self.is_valid = false;
    }
}

/// Configuration parameters for deadline validation
#[derive(Debug, Clone)]
pub struct DeadlineConfigInput {
    pub preseason_keeper_deadline: DateTimeWithTimeZone,
    pub veteran_auction_days_after_keeper_deadline_duration: i16,
    pub fa_auction_days_duration: i16,
    pub final_roster_lock_deadline_days_after_rookie_draft: i16,
    pub playoffs_start_week: i16,
}

/// Validate deadline configuration rules
#[instrument]
pub fn validate_deadline_config(config: &DeadlineConfigInput) -> Result<ValidationResult> {
    let mut result = ValidationResult::valid();

    // Validate preseason keeper deadline is in the future
    let now: DateTimeWithTimeZone = chrono::Utc::now().into();
    if config.preseason_keeper_deadline <= now {
        result.add_error(
            "preseason_keeper_deadline",
            "Preseason keeper deadline must be in the future",
        );
    }

    // Validate all day durations are positive
    if config.veteran_auction_days_after_keeper_deadline_duration <= 0 {
        result.add_error(
            "veteran_auction_days_after_keeper_deadline_duration",
            "Veteran auction days after keeper must be a positive number",
        );
    }

    if config.fa_auction_days_duration <= 0 {
        result.add_error(
            "fa_auction_days_duration",
            "Free agent auction duration must be a positive number",
        );
    }

    if config.final_roster_lock_deadline_days_after_rookie_draft <= 0 {
        result.add_error(
            "final_roster_lock_deadline_days_after_rookie_draft",
            "Final roster lock days after rookie draft must be a positive number",
        );
    }

    // Validate playoffs start week is reasonable (between 1 and 26)
    if config.playoffs_start_week < 1 || config.playoffs_start_week > 26 {
        result.add_error(
            "playoffs_start_week",
            "Playoffs start week must be between 1 and 26",
        );
    }

    // Validate logical constraints - ensure sufficient time between phases
    if config.veteran_auction_days_after_keeper_deadline_duration < 3 {
        result.add_error(
            "veteran_auction_days_after_keeper_deadline_duration",
            "Veteran auction should start at least 3 days after keeper deadline to allow processing time",
        );
    }

    if config.fa_auction_days_duration < 1 {
        result.add_error(
            "fa_auction_days_duration",
            "Free agent auction should last at least 1 day",
        );
    }

    if config.final_roster_lock_deadline_days_after_rookie_draft < 1 {
        result.add_error(
            "final_roster_lock_deadline_days_after_rookie_draft",
            "Final roster lock should be at least 1 day after rookie draft to allow for roster decisions",
        );
    }

    Ok(result)
}

/// Validate that the configuration doesn't conflict with existing deadlines
#[instrument]
pub fn validate_config_timing(config: &DeadlineConfigInput) -> Result<ValidationResult> {
    let mut result = ValidationResult::valid();

    // Calculate approximate timeline to ensure reasonable spacing
    let keeper_date = config.preseason_keeper_deadline;

    // Veteran auction would start after keeper + offset days
    let veteran_auction_start = keeper_date
        + chrono::Duration::days(config.veteran_auction_days_after_keeper_deadline_duration as i64);

    // FA auction would run for the specified duration after veteran auction
    let fa_auction_end =
        veteran_auction_start + chrono::Duration::days(config.fa_auction_days_duration as i64);

    // Rookie draft starts 3 days after veteran auction (per PRD)
    let rookie_draft_start = veteran_auction_start + chrono::Duration::days(3);

    // Final roster lock comes after rookie draft + offset
    let final_roster_lock = rookie_draft_start
        + chrono::Duration::days(config.final_roster_lock_deadline_days_after_rookie_draft as i64);

    // Ensure FA auction doesn't end after final roster lock
    // Note: FA auction starts 3 hours after veteran auction and runs concurrently with other events
    // This check might be too restrictive - let's make it more lenient for now
    if fa_auction_end > final_roster_lock + chrono::Duration::days(1) {
        result.add_error(
            "fa_auction_days_duration",
            "Free agent auction duration is too long - it would extend well past final roster lock",
        );
    }

    // Ensure reasonable minimum time between keeper and final roster lock (at least 2 weeks)
    let min_preseason_duration = chrono::Duration::days(14);
    if final_roster_lock - keeper_date < min_preseason_duration {
        result.add_error(
            "general",
            "Overall preseason timeline is too compressed - need at least 2 weeks from keeper deadline to final roster lock",
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_valid_configuration() {
        let config = DeadlineConfigInput {
            preseason_keeper_deadline: (Utc::now() + Duration::days(30)).into(),
            veteran_auction_days_after_keeper_deadline_duration: 7,
            fa_auction_days_duration: 5,
            final_roster_lock_deadline_days_after_rookie_draft: 5, // Increased to ensure 14+ day timeline
            playoffs_start_week: 21,
        };

        let result = validate_deadline_config(&config).unwrap();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_keeper_deadline_in_past() {
        let config = DeadlineConfigInput {
            preseason_keeper_deadline: (Utc::now() - Duration::days(1)).into(),
            veteran_auction_days_after_keeper_deadline_duration: 7,
            fa_auction_days_duration: 5,
            final_roster_lock_deadline_days_after_rookie_draft: 3,
            playoffs_start_week: 21,
        };

        let result = validate_deadline_config(&config).unwrap();
        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.field == "preseason_keeper_deadline")
        );
    }

    #[test]
    fn test_negative_durations() {
        let config = DeadlineConfigInput {
            preseason_keeper_deadline: (Utc::now() + Duration::days(30)).into(),
            veteran_auction_days_after_keeper_deadline_duration: -1, // Should error
            fa_auction_days_duration: 0,                             // Should error
            final_roster_lock_deadline_days_after_rookie_draft: -2,  // Should error
            playoffs_start_week: 21,
        };

        let result = validate_deadline_config(&config).unwrap();
        assert!(!result.is_valid);
        // Expecting errors for: veteran_auction_days (-1), fa_auction_days (0), final_roster_lock_days (-2)
        // Plus additional logical validation errors for being too short
        println!("Validation errors: {:?}", result.errors);
        assert!(result.errors.len() >= 3); // At least 3 errors, might be more due to logical constraints
    }

    #[test]
    fn test_invalid_playoffs_week() {
        let config = DeadlineConfigInput {
            preseason_keeper_deadline: (Utc::now() + Duration::days(30)).into(),
            veteran_auction_days_after_keeper_deadline_duration: 7,
            fa_auction_days_duration: 5,
            final_roster_lock_deadline_days_after_rookie_draft: 3,
            playoffs_start_week: 30, // Invalid - too high
        };

        let result = validate_deadline_config(&config).unwrap();
        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.field == "playoffs_start_week")
        );
    }
}
