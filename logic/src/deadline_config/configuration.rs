use color_eyre::Result;
use fbkl_entity::{
    deadline, deadline_config_rule,
    deadline_config_rule_queries::{self, UpsertConfigParams},
    sea_orm::ConnectionTrait,
};
use std::fmt::Debug;
use tracing::instrument;

use crate::deadline_config::{generation, validation::DeadlineConfigInput};

/// Main function to configure deadlines for a league season
#[instrument]
pub async fn configure_deadlines<C>(
    league_id: i64,
    end_of_season_year: i16,
    config_input: DeadlineConfigInput,
    db: &C,
) -> Result<deadline_config_rule::Model>
where
    C: ConnectionTrait + Debug,
{
    // 1. Validate the configuration
    let validation_result =
        crate::deadline_config::validation::validate_deadline_config(&config_input)?;
    if !validation_result.is_valid {
        return Err(color_eyre::eyre::eyre!(
            "Configuration validation failed: {:?}",
            validation_result.errors
        ));
    }

    // 2. Validate timing constraints
    let timing_validation =
        crate::deadline_config::validation::validate_config_timing(&config_input)?;
    if !timing_validation.is_valid {
        return Err(color_eyre::eyre::eyre!(
            "Configuration timing validation failed: {:?}",
            timing_validation.errors
        ));
    }

    // 3. Save the configuration
    let params = UpsertConfigParams {
        league_id,
        end_of_season_year,
        preseason_keeper_deadline: config_input.preseason_keeper_deadline,
        veteran_auction_days_after_keeper_deadline_duration: config_input
            .veteran_auction_days_after_keeper_deadline_duration,
        fa_auction_days_duration: config_input.fa_auction_days_duration,
        final_roster_lock_deadline_days_after_rookie_draft: config_input
            .final_roster_lock_deadline_days_after_rookie_draft,
        playoffs_start_week: config_input.playoffs_start_week,
    };

    let config_model = deadline_config_rule_queries::upsert_config_rules(params, db).await?;

    Ok(config_model)
}

/// Activate deadlines for a league season (generates deadline records)
#[instrument]
pub async fn activate_deadlines<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<deadline::Model>>
where
    C: ConnectionTrait + Debug,
{
    // 1. Get the configuration
    let config = deadline_config_rule_queries::get_config_for_league_season(
        league_id,
        end_of_season_year,
        db,
    )
    .await?
    .ok_or_else(|| {
        color_eyre::eyre::eyre!(
            "No deadline configuration found for league {} season {}",
            league_id,
            end_of_season_year
        )
    })?;

    // 2. Check if deadlines can still be edited (will be enhanced in Phase 2)
    // For now, always allow activation

    // 3. Generate deadlines from configuration
    let generated_deadlines = generation::generate_deadlines_from_config(&config)?;

    // 4. Save deadlines to database
    let deadlines = generation::save_generated_deadlines(
        generated_deadlines,
        league_id,
        end_of_season_year,
        db,
    )
    .await?;

    Ok(deadlines)
}

/// Get deadline configuration for a league season
#[instrument]
pub async fn get_deadline_config<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Option<deadline_config_rule::Model>>
where
    C: ConnectionTrait + Debug,
{
    deadline_config_rule_queries::get_config_for_league_season(league_id, end_of_season_year, db)
        .await
}

/// Check if deadline configuration can still be edited
#[instrument]
pub async fn can_edit_config<C>(league_id: i64, end_of_season_year: i16, db: &C) -> Result<bool>
where
    C: ConnectionTrait + Debug,
{
    // For Phase 1, always allow editing (no status checking yet)
    // In Phase 2, this will check deadline statuses:
    // - Draft: All deadlines editable
    // - Activated: Only future deadlines editable
    // - Processed: No editing allowed

    let _config = get_deadline_config(league_id, end_of_season_year, db).await?;

    // TODO: In Phase 2, check deadline statuses and current time
    // For now, return true if config exists
    Ok(true)
}

/// Delete deadline configuration (for testing/admin purposes)
#[instrument]
pub async fn delete_deadline_config<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<bool>
where
    C: ConnectionTrait + Debug,
{
    use fbkl_entity::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let delete_result = deadline_config_rule::Entity::delete_many()
        .filter(deadline_config_rule::Column::LeagueId.eq(league_id))
        .filter(deadline_config_rule::Column::EndOfSeasonYear.eq(end_of_season_year))
        .exec(db)
        .await?;

    Ok(delete_result.rows_affected > 0)
}

#[cfg(test)]
mod tests {
    use crate::deadline_config::validation::DeadlineConfigInput;
    use chrono::{Duration, Utc};

    // Note: These tests would require a test database setup
    // For now, we'll include unit tests for the validation logic

    #[test]
    fn test_validation_integration() {
        let valid_config = DeadlineConfigInput {
            preseason_keeper_deadline: (Utc::now() + Duration::days(30)).into(),
            veteran_auction_days_after_keeper_deadline_duration: 7,
            fa_auction_days_duration: 5,
            final_roster_lock_deadline_days_after_rookie_draft: 5, // Increased from 3 to 5 days
            playoffs_start_week: 21,
        };

        let validation_result =
            crate::deadline_config::validation::validate_deadline_config(&valid_config).unwrap();
        assert!(validation_result.is_valid);

        let timing_result =
            crate::deadline_config::validation::validate_config_timing(&valid_config).unwrap();
        if !timing_result.is_valid {
            println!("Timing validation errors: {:?}", timing_result.errors);
        }
        assert!(timing_result.is_valid);
    }

    #[test]
    fn test_invalid_config_handling() {
        let invalid_config = DeadlineConfigInput {
            preseason_keeper_deadline: (Utc::now() - Duration::days(1)).into(), // Past date
            veteran_auction_days_after_keeper_deadline_duration: -1,            // Negative
            fa_auction_days_duration: 0,                                        // Zero
            final_roster_lock_deadline_days_after_rookie_draft: -5,             // Negative
            playoffs_start_week: 50,                                            // Too high
        };

        let validation_result =
            crate::deadline_config::validation::validate_deadline_config(&invalid_config).unwrap();
        assert!(!validation_result.is_valid);
        assert!(!validation_result.errors.is_empty());
    }
}
