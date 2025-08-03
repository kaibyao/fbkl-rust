use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
    prelude::DateTimeWithTimeZone,
};
use tracing::instrument;

use crate::deadline_config_rule::{self, ActiveModel};

/// Parameters for upserting deadline configuration rules
#[derive(Debug, Clone)]
pub struct UpsertConfigParams {
    pub league_id: i64,
    pub end_of_season_year: i16,
    pub preseason_keeper_deadline: DateTimeWithTimeZone,
    pub veteran_auction_days_after_keeper_deadline_duration: i16,
    pub fa_auction_days_duration: i16,
    pub final_roster_lock_deadline_days_after_rookie_draft: i16,
    pub playoffs_start_week: i16,
}

/// Get deadline configuration rules for a specific league and season
#[instrument]
pub async fn get_config_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Option<deadline_config_rule::Model>>
where
    C: ConnectionTrait + Debug,
{
    let config = deadline_config_rule::Entity::find()
        .filter(deadline_config_rule::Column::LeagueId.eq(league_id))
        .filter(deadline_config_rule::Column::EndOfSeasonYear.eq(end_of_season_year))
        .one(db)
        .await?;

    Ok(config)
}

/// Upsert (insert or update) deadline configuration rules for a league season
#[instrument]
pub async fn upsert_config_rules<C>(
    params: UpsertConfigParams,
    db: &C,
) -> Result<deadline_config_rule::Model>
where
    C: ConnectionTrait + Debug,
{
    // Try to find existing configuration
    let existing_config =
        get_config_for_league_season(params.league_id, params.end_of_season_year, db).await?;

    let config_model = if let Some(existing) = existing_config {
        // Update existing configuration
        let mut active_model: ActiveModel = existing.into();
        active_model.preseason_keeper_deadline = Set(params.preseason_keeper_deadline);
        active_model.veteran_auction_days_after_keeper_deadline_duration =
            Set(params.veteran_auction_days_after_keeper_deadline_duration);
        active_model.fa_auction_days_duration = Set(params.fa_auction_days_duration);
        active_model.final_roster_lock_deadline_days_after_rookie_draft =
            Set(params.final_roster_lock_deadline_days_after_rookie_draft);
        active_model.playoffs_start_week = Set(params.playoffs_start_week);
        active_model.updated_at = Set(chrono::Utc::now().into());

        active_model.update(db).await?
    } else {
        // Create new configuration
        let new_config = ActiveModel {
            league_id: Set(params.league_id),
            end_of_season_year: Set(params.end_of_season_year),
            preseason_keeper_deadline: Set(params.preseason_keeper_deadline),
            veteran_auction_days_after_keeper_deadline_duration: Set(
                params.veteran_auction_days_after_keeper_deadline_duration
            ),
            fa_auction_days_duration: Set(params.fa_auction_days_duration),
            final_roster_lock_deadline_days_after_rookie_draft: Set(
                params.final_roster_lock_deadline_days_after_rookie_draft
            ),
            playoffs_start_week: Set(params.playoffs_start_week),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
            ..Default::default()
        };

        new_config.insert(db).await?
    };

    Ok(config_model)
}

/// Get all activated deadline configurations (for processing engine)
#[instrument]
pub async fn get_all_activated_configs<C>(db: &C) -> Result<Vec<deadline_config_rule::Model>>
where
    C: ConnectionTrait + Debug,
{
    // Note: This will be used later when we implement processing engine
    // For now, return all configs - we'll add status filtering when deadlines are linked
    let configs = deadline_config_rule::Entity::find().all(db).await?;

    Ok(configs)
}
