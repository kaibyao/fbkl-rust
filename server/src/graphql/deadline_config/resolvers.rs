use async_graphql::{Context, Error as GraphQlError, Object, Result};
use axum::http::StatusCode;
use fbkl_entity::{
    deadline,
    deadline_queries::find_deadlines_by_date_for_league_season,
    league_queries::find_league_by_user,
    sea_orm::{DatabaseConnection, EntityTrait},
    team_user::LeagueRole,
    team_user_queries::get_team_user_by_user_and_league,
    user,
};
use fbkl_jobs;
use fbkl_logic::deadline_config::{
    DeadlineConfigInput, activate_deadlines, can_edit_config, configure_deadlines,
    get_deadline_config,
};
use fbkl_logic::deadline_processing::process_single_deadline;
use tower_sessions::Session;

use crate::session::enforce_logged_in;

use super::types::{
    Deadline, DeadlineConfigRules, DeadlineConfigRulesInput, DeadlineConfigStatus, ProcessingReport,
};

/// GraphQL query resolvers for deadline configuration
#[derive(Default)]
pub struct DeadlineConfigQuery;

#[Object]
impl DeadlineConfigQuery {
    /// Get deadline configuration rules for a league and season
    async fn deadline_config_rules(
        &self,
        ctx: &Context<'_>,
        league_id: i64,
        end_of_season_year: i16,
    ) -> Result<Option<DeadlineConfigRules>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in
        let _user_id = enforce_logged_in(session.clone()).await?;

        // Verify user has access to this league
        let user_model = match ctx.data_unchecked::<Option<user::Model>>() {
            None => return Err(GraphQlError::from(StatusCode::UNAUTHORIZED)),
            Some(user) => user,
        };

        match find_league_by_user(user_model, league_id, db).await {
            Ok(None) => return Err(GraphQlError::from(StatusCode::NOT_FOUND)),
            Ok(Some(_)) => {} // User has access to league
            Err(_) => return Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }

        // Get deadline configuration
        match get_deadline_config(league_id, end_of_season_year, db).await {
            Ok(Some(config)) => Ok(Some(DeadlineConfigRules::from_model(config))),
            Ok(None) => Ok(None),
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// Get status information for deadline configuration
    async fn deadline_config_status(
        &self,
        ctx: &Context<'_>,
        league_id: i64,
        end_of_season_year: i16,
    ) -> Result<DeadlineConfigStatus> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in
        let _user_id = enforce_logged_in(session.clone()).await?;

        // Verify user has access to this league
        let user_model = match ctx.data_unchecked::<Option<user::Model>>() {
            None => return Err(GraphQlError::from(StatusCode::UNAUTHORIZED)),
            Some(user) => user,
        };

        match find_league_by_user(user_model, league_id, db).await {
            Ok(None) => return Err(GraphQlError::from(StatusCode::NOT_FOUND)),
            Ok(Some(_)) => {} // User has access to league
            Err(_) => return Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }

        // Check if configuration exists
        let has_configuration = match get_deadline_config(league_id, end_of_season_year, db).await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(_) => return Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        };

        // Get deadlines and count by status
        let deadline_map =
            match find_deadlines_by_date_for_league_season(league_id, end_of_season_year, db).await
            {
                Ok(deadlines) => deadlines,
                Err(_) => return Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
            };
        let deadlines: Vec<deadline::Model> = deadline_map.into_values().collect();

        let mut draft_count = 0;
        let mut activated_count = 0;
        let mut processed_count = 0;

        for deadline in &deadlines {
            match deadline.status {
                deadline::DeadlineStatus::Draft => draft_count += 1,
                deadline::DeadlineStatus::Activated => activated_count += 1,
                deadline::DeadlineStatus::Processing | deadline::DeadlineStatus::Processed => {
                    processed_count += 1
                }
                deadline::DeadlineStatus::Error => {} // Don't count error status
            }
        }

        let is_activated = activated_count > 0 || processed_count > 0;

        // Check if user can edit (must be commissioner and config must be editable)
        let can_edit = match self.check_commissioner_access(ctx, league_id).await {
            Ok(true) => {
                // User is commissioner, check if configuration is editable
                can_edit_config(league_id, end_of_season_year, db)
                    .await
                    .unwrap_or_default()
            }
            _ => false,
        };

        Ok(DeadlineConfigStatus {
            has_configuration,
            is_activated,
            draft_deadlines_count: draft_count,
            activated_deadlines_count: activated_count,
            processed_deadlines_count: processed_count,
            can_edit,
        })
    }

    /// Get deadlines for a league and season (enhanced with status)
    async fn deadlines(
        &self,
        ctx: &Context<'_>,
        league_id: i64,
        end_of_season_year: i16,
    ) -> Result<Vec<Deadline>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in
        let _user_id = enforce_logged_in(session.clone()).await?;

        // Verify user has access to this league
        let user_model = match ctx.data_unchecked::<Option<user::Model>>() {
            None => return Err(GraphQlError::from(StatusCode::UNAUTHORIZED)),
            Some(user) => user,
        };

        match find_league_by_user(user_model, league_id, db).await {
            Ok(None) => return Err(GraphQlError::from(StatusCode::NOT_FOUND)),
            Ok(Some(_)) => {} // User has access to league
            Err(_) => return Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }

        // Get deadlines
        match find_deadlines_by_date_for_league_season(league_id, end_of_season_year, db).await {
            Ok(deadline_map) => {
                let deadline_types = deadline_map
                    .into_values()
                    .map(Deadline::from_model)
                    .collect();
                Ok(deadline_types)
            }
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }
}

impl DeadlineConfigQuery {
    /// Helper method to check if current user is a commissioner for the league
    async fn check_commissioner_access(&self, ctx: &Context<'_>, league_id: i64) -> Result<bool> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in
        let _user_id = enforce_logged_in(session.clone()).await?;

        // Get user's team_user record for this league
        match get_team_user_by_user_and_league(&_user_id, &league_id, db).await {
            Ok(Some((team_user, _))) => Ok(team_user.league_role == LeagueRole::LeagueCommissioner),
            Ok(None) => Ok(false), // User not in league
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }
}

/// GraphQL mutation resolvers for deadline configuration
#[derive(Default)]
pub struct DeadlineConfigMutation;

#[Object]
impl DeadlineConfigMutation {
    /// Configure deadline rules for a league and season
    async fn configure_season_deadlines(
        &self,
        ctx: &Context<'_>,
        league_id: i64,
        end_of_season_year: i16,
        config: DeadlineConfigRulesInput,
    ) -> Result<DeadlineConfigRules> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in and is commissioner for this league
        let _user_id = enforce_logged_in(session.clone()).await?;

        if !self.check_commissioner_access(ctx, league_id).await? {
            return Err(GraphQlError::from(StatusCode::FORBIDDEN));
        }

        // Convert GraphQL input to business logic input
        let preseason_keeper_deadline = config
            .preseason_keeper_deadline
            .parse()
            .map_err(|_| GraphQlError::from(StatusCode::BAD_REQUEST))?;

        let deadline_config_input = DeadlineConfigInput {
            preseason_keeper_deadline,
            veteran_auction_days_after_keeper_deadline_duration: config
                .veteran_auction_days_after_keeper_deadline_duration,
            fa_auction_days_duration: config.fa_auction_days_duration,
            final_roster_lock_deadline_days_after_rookie_draft: config
                .final_roster_lock_deadline_days_after_rookie_draft,
            playoffs_start_week: config.playoffs_start_week,
        };

        // Configure deadlines via business logic
        match configure_deadlines(league_id, end_of_season_year, deadline_config_input, db).await {
            Ok(config_model) => Ok(DeadlineConfigRules::from_model(config_model)),
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// Activate deadline configuration for a league and season
    async fn activate_season_deadlines(
        &self,
        ctx: &Context<'_>,
        league_id: i64,
        end_of_season_year: i16,
    ) -> Result<bool> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in and is commissioner for this league
        let _user_id = enforce_logged_in(session.clone()).await?;

        if !self.check_commissioner_access(ctx, league_id).await? {
            return Err(GraphQlError::from(StatusCode::FORBIDDEN));
        }

        // Activate deadlines via business logic
        match activate_deadlines(league_id, end_of_season_year, db).await {
            Ok(_) => Ok(true),
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// Manually process a specific deadline (admin only)
    async fn process_deadline_now(&self, ctx: &Context<'_>, deadline_id: i64) -> Result<bool> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in and is an admin
        let _user_id = enforce_logged_in(session.clone()).await?;

        if !self.check_admin_access(ctx).await? {
            return Err(GraphQlError::from(StatusCode::FORBIDDEN));
        }

        // Find the specific deadline
        let deadline = match deadline::Entity::find_by_id(deadline_id).one(db).await {
            Ok(Some(deadline)) => deadline,
            Ok(None) => return Err(GraphQlError::from(StatusCode::NOT_FOUND)),
            Err(_) => return Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        };

        // Process the deadline
        match process_single_deadline(&deadline, db).await {
            Ok(result) => Ok(result.success),
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// Process all ready deadlines for the system (admin only)
    async fn process_all_ready_deadlines(&self, ctx: &Context<'_>) -> Result<ProcessingReport> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in and is an admin
        let _user_id = enforce_logged_in(session.clone()).await?;

        if !self.check_admin_access(ctx).await? {
            return Err(GraphQlError::from(StatusCode::FORBIDDEN));
        }

        // Process all deadlines using the jobs crate
        match fbkl_jobs::process_deadlines(db).await {
            Ok(report) => Ok(ProcessingReport::from(report)),
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// Process all ready deadlines for a specific league/season (admin only)
    async fn process_league_deadlines(
        &self,
        ctx: &Context<'_>,
        league_id: i64,
        end_of_season_year: i16,
    ) -> Result<ProcessingReport> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in and is an admin
        let _user_id = enforce_logged_in(session.clone()).await?;

        if !self.check_admin_access(ctx).await? {
            return Err(GraphQlError::from(StatusCode::FORBIDDEN));
        }

        // Create a minimal processing report for league-specific processing
        let mut report = fbkl_jobs::ProcessingReport::new();

        // Get deadlines for this league/season that are ready for processing
        let deadline_map =
            match find_deadlines_by_date_for_league_season(league_id, end_of_season_year, db).await
            {
                Ok(deadlines) => deadlines,
                Err(_) => return Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
            };

        let ready_deadlines: Vec<deadline::Model> = deadline_map
            .into_values()
            .filter(|d| d.status == deadline::DeadlineStatus::Activated)
            .collect();

        if ready_deadlines.is_empty() {
            return Ok(ProcessingReport::from(report.complete()));
        }

        // Process each deadline
        for deadline in ready_deadlines {
            match process_single_deadline(&deadline, db).await {
                Ok(result) => {
                    report.deadlines_processed.push(result);
                }
                Err(e) => {
                    report
                        .errors
                        .push(format!("Failed to process deadline {}: {}", deadline.id, e));
                }
            }
        }

        Ok(ProcessingReport::from(report.complete()))
    }
}

impl DeadlineConfigMutation {
    /// Helper method to check if current user is a commissioner for the league
    async fn check_commissioner_access(&self, ctx: &Context<'_>, league_id: i64) -> Result<bool> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in
        let _user_id = enforce_logged_in(session.clone()).await?;

        // Get user's team_user record for this league
        match get_team_user_by_user_and_league(&_user_id, &league_id, db).await {
            Ok(Some((team_user, _))) => Ok(team_user.league_role == LeagueRole::LeagueCommissioner),
            Ok(None) => Ok(false), // User not in league
            Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
        }
    }

    /// Helper method to check if current user is an admin
    async fn check_admin_access(&self, ctx: &Context<'_>) -> Result<bool> {
        let session = ctx.data_unchecked::<Session>();

        // Verify user is logged in
        let _user_id = enforce_logged_in(session.clone()).await?;

        // Get user model from context
        let user_model = match ctx.data_unchecked::<Option<user::Model>>() {
            None => return Err(GraphQlError::from(StatusCode::UNAUTHORIZED)),
            Some(user) => user,
        };

        // Check if user has admin status
        Ok(user_model.app_admin_status == user::UserAppAdminStatus::Admin)
    }
}
