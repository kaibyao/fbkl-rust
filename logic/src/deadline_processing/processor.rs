use chrono::Utc;
use color_eyre::Result;
use fbkl_entity::{
    deadline::{self, DeadlineKind, DeadlineStatus},
    deadline_config_rule, deadline_queries,
    sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait},
};
use std::{collections::HashMap, fmt::Debug};
use tracing::{debug, error, info, instrument, warn};

use crate::deadline_config::generation::{
    generate_deadlines_from_config, get_deadline_dependency_order, save_generated_deadlines,
};
use crate::deadline_processing::{
    keeper_deadline::process_keeper_deadline_transaction, roster_lock::lock_rosters,
};

/// Result of processing operation
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    pub deadline_id: i64,
    pub deadline_kind: deadline::DeadlineKind,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Generate deadline records from activated configuration rules for all leagues that need them
#[instrument]
pub async fn generate_deadlines_if_needed<C>(
    db: &C,
) -> Result<Vec<(i64, i16, Vec<deadline::Model>)>>
where
    C: ConnectionTrait + Debug,
{
    info!("Starting deadline generation for leagues with configurations but no deadlines");

    // Get all deadline config rules
    let all_configs = deadline_config_rule::Entity::find().all(db).await?;

    let mut results = Vec::new();

    for config in all_configs {
        // Check if we already have deadlines for this league/season
        let existing_deadlines = deadline_queries::find_deadlines_by_date_for_league_season(
            config.league_id,
            config.end_of_season_year,
            db,
        )
        .await?;

        // Only generate if no deadlines exist yet
        if existing_deadlines.is_empty() {
            info!(
                "Generating deadlines for league {} season {}",
                config.league_id, config.end_of_season_year
            );

            // Generate deadlines from config
            let generated_deadlines = generate_deadlines_from_config(&config)?;

            // Create deadlines with Activated status (ready for processing)
            let mut activated_deadlines = generated_deadlines;
            for deadline in &mut activated_deadlines {
                deadline.status = DeadlineStatus::Activated;
            }

            // Save to database
            let saved_deadlines = save_generated_deadlines(
                activated_deadlines,
                config.league_id,
                config.end_of_season_year,
                db,
            )
            .await?;

            info!(
                "Generated {} deadlines for league {} season {}",
                saved_deadlines.len(),
                config.league_id,
                config.end_of_season_year
            );

            results.push((config.league_id, config.end_of_season_year, saved_deadlines));
        } else {
            // Check if config has been updated since deadlines were created
            let config_updated = config.updated_at;
            let oldest_deadline_created = existing_deadlines
                .values()
                .map(|d| d.created_at)
                .min()
                .unwrap_or(config_updated);

            if config_updated > oldest_deadline_created {
                warn!(
                    "Config updated after deadlines created for league {} season {} - consider cleanup",
                    config.league_id, config.end_of_season_year
                );

                // For now, just log - in future versions we might implement automatic cleanup
                // This would require careful handling of processed deadlines
            }
        }
    }

    info!(
        "Completed deadline generation - processed {} configurations",
        results.len()
    );
    Ok(results)
}

/// Clean up outdated deadlines when configuration has been updated
/// This is a preparatory function for future use - currently just validates the approach
#[instrument]
pub async fn cleanup_outdated_deadlines<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    info!(
        "Checking for outdated deadlines for league {} season {}",
        league_id, end_of_season_year
    );

    // Get existing deadlines
    let existing_deadlines = deadline_queries::find_deadlines_by_date_for_league_season(
        league_id,
        end_of_season_year,
        db,
    )
    .await?;

    // Count deadlines by status
    let mut draft_count = 0;
    let mut activated_count = 0;
    let mut processing_count = 0;
    let mut processed_count = 0;
    let mut error_count = 0;

    for deadline in existing_deadlines.values() {
        match deadline.status {
            DeadlineStatus::Draft => draft_count += 1,
            DeadlineStatus::Activated => activated_count += 1,
            DeadlineStatus::Processing => processing_count += 1,
            DeadlineStatus::Processed => processed_count += 1,
            DeadlineStatus::Error => error_count += 1,
        }
    }

    info!(
        "Deadline status counts - Draft: {}, Activated: {}, Processing: {}, Processed: {}, Error: {}",
        draft_count, activated_count, processing_count, processed_count, error_count
    );

    // For Phase 3, we only log the analysis
    // Future phases will implement actual cleanup logic based on status
    if processed_count > 0 {
        warn!(
            "Found {} processed deadlines - cleanup would require careful rollback handling",
            processed_count
        );
    }

    if processing_count > 0 {
        error!(
            "Found {} deadlines in processing state - these should be investigated",
            processing_count
        );
    }

    Ok(())
}

/// Process activated deadlines that are ready (current time >= deadline time)
/// This function finds all deadlines with Activated status where the deadline time has passed
/// and processes them in dependency order to ensure prerequisites are met
#[instrument]
pub async fn process_activated_deadlines<C>(db: &C) -> Result<Vec<ProcessingResult>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    info!("Starting processing of activated deadlines");

    let now = Utc::now();
    let mut results = Vec::new();

    // Get all activated deadlines that are ready to process (deadline time has passed)
    let ready_deadlines = deadline_queries::find_deadlines_by_status(DeadlineStatus::Activated, db)
        .await?
        .into_iter()
        .filter(|deadline| deadline.date_time <= now)
        .collect::<Vec<_>>();

    if ready_deadlines.is_empty() {
        info!("No activated deadlines ready for processing");
        return Ok(results);
    }

    info!("Found {} ready deadlines to process", ready_deadlines.len());

    // Group deadlines by league and season for dependency processing
    let mut deadlines_by_league_season: HashMap<(i64, i16), Vec<deadline::Model>> = HashMap::new();

    for deadline in ready_deadlines {
        let key = (deadline.league_id, deadline.end_of_season_year);
        deadlines_by_league_season
            .entry(key)
            .or_default()
            .push(deadline);
    }

    // Process each league/season group independently
    for ((league_id, end_of_season_year), mut deadlines) in deadlines_by_league_season {
        info!(
            "Processing {} deadlines for league {} season {}",
            deadlines.len(),
            league_id,
            end_of_season_year
        );

        // Sort deadlines by dependency order
        let dependency_order = get_deadline_dependency_order();
        deadlines.sort_by_key(|deadline| {
            dependency_order
                .iter()
                .position(|&kind| kind == deadline.kind)
                .unwrap_or(usize::MAX)
        });

        // Process each deadline in order
        for deadline in deadlines {
            let result = process_single_deadline(&deadline, db).await;

            match result {
                Ok(processing_result) => {
                    info!(
                        "Successfully processed deadline {} ({:?})",
                        deadline.id, deadline.kind
                    );
                    results.push(processing_result);
                }
                Err(e) => {
                    error!(
                        "Failed to process deadline {} ({:?}): {}",
                        deadline.id, deadline.kind, e
                    );

                    // Create error result and continue with other deadlines
                    let error_result = ProcessingResult {
                        deadline_id: deadline.id,
                        deadline_kind: deadline.kind,
                        success: false,
                        error_message: Some(e.to_string()),
                    };
                    results.push(error_result);
                }
            }
        }
    }

    info!(
        "Completed processing activated deadlines - processed {} total",
        results.len()
    );
    Ok(results)
}

/// Process a single deadline, checking prerequisites and updating status
#[instrument]
pub async fn process_single_deadline<C>(
    deadline: &deadline::Model,
    db: &C,
) -> Result<ProcessingResult>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    info!("Processing deadline {} ({:?})", deadline.id, deadline.kind);

    // Check prerequisites first
    let prerequisites_met = check_deadline_prerequisites(deadline, db).await?;
    if !prerequisites_met {
        warn!(
            "Prerequisites not met for deadline {} ({:?}), skipping",
            deadline.id, deadline.kind
        );
        return Ok(ProcessingResult {
            deadline_id: deadline.id,
            deadline_kind: deadline.kind,
            success: false,
            error_message: Some("Prerequisites not met".to_string()),
        });
    }

    // Transition to Processing status
    transition_deadline_status(deadline.id, DeadlineStatus::Processing, None, db).await?;

    // Process the deadline based on its kind
    let processing_result = match deadline.kind {
        DeadlineKind::PreseasonKeeper => {
            debug!("Processing keeper deadline");
            match process_keeper_deadline_transaction(
                deadline.league_id,
                deadline.end_of_season_year,
                db,
            )
            .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        }
        DeadlineKind::PreseasonFinalRosterLock
        | DeadlineKind::Week1RosterLock
        | DeadlineKind::InSeasonRosterLock => {
            debug!("Processing roster lock deadline");
            lock_rosters(deadline, db).await
        }
        DeadlineKind::PreseasonVeteranAuctionStart
        | DeadlineKind::PreseasonFaAuctionStart
        | DeadlineKind::PreseasonFaAuctionEnd
        | DeadlineKind::PreseasonRookieDraftStart => {
            // These deadlines don't require immediate processing - they're triggers for other systems
            debug!(
                "Deadline {} ({:?}) is a trigger deadline, marking as processed",
                deadline.id, deadline.kind
            );
            Ok(())
        }
        _ => {
            warn!(
                "Unsupported deadline kind for processing: {:?}",
                deadline.kind
            );
            Ok(()) // Mark as processed even if we don't handle it
        }
    };

    // Update final status based on processing result
    match processing_result {
        Ok(_) => {
            transition_deadline_status(deadline.id, DeadlineStatus::Processed, None, db).await?;
            info!(
                "Successfully processed deadline {} ({:?})",
                deadline.id, deadline.kind
            );
            Ok(ProcessingResult {
                deadline_id: deadline.id,
                deadline_kind: deadline.kind,
                success: true,
                error_message: None,
            })
        }
        Err(e) => {
            let error_msg = e.to_string();
            transition_deadline_status(
                deadline.id,
                DeadlineStatus::Error,
                Some(error_msg.clone()),
                db,
            )
            .await?;
            error!(
                "Failed to process deadline {} ({:?}): {}",
                deadline.id, deadline.kind, error_msg
            );
            Ok(ProcessingResult {
                deadline_id: deadline.id,
                deadline_kind: deadline.kind,
                success: false,
                error_message: Some(error_msg),
            })
        }
    }
}

/// Check if prerequisites are met for processing a deadline
/// This ensures deadlines are processed in the correct dependency order
#[instrument]
pub async fn check_deadline_prerequisites<C>(deadline: &deadline::Model, db: &C) -> Result<bool>
where
    C: ConnectionTrait + Debug,
{
    debug!(
        "Checking prerequisites for deadline {} ({:?})",
        deadline.id, deadline.kind
    );

    let dependency_order = get_deadline_dependency_order();

    // Find the position of this deadline in the dependency order
    let current_position = dependency_order
        .iter()
        .position(|&kind| kind == deadline.kind);

    let current_position = match current_position {
        Some(pos) => pos,
        None => {
            // If not in dependency order, allow processing (might be a manually created deadline)
            debug!(
                "Deadline kind {:?} not in dependency order, allowing processing",
                deadline.kind
            );
            return Ok(true);
        }
    };

    // Check that all previous deadlines in the dependency chain are processed
    for i in 0..current_position {
        let required_kind = dependency_order[i];

        // Find the deadline of this required kind for the same league/season
        match deadline_queries::find_deadline_for_season_by_type(
            deadline.league_id,
            deadline.end_of_season_year,
            required_kind,
            db,
        )
        .await
        {
            Ok(required_deadline) => {
                if required_deadline.status != DeadlineStatus::Processed {
                    info!(
                        "Prerequisites not met: {:?} ({}) requires {:?} to be processed (current status: {:?})",
                        deadline.kind, deadline.id, required_kind, required_deadline.status
                    );
                    return Ok(false);
                }
            }
            Err(_) => {
                // Required deadline doesn't exist - this might be okay for some cases
                debug!(
                    "Required deadline {:?} not found for league {} season {}, continuing",
                    required_kind, deadline.league_id, deadline.end_of_season_year
                );
            }
        }
    }

    debug!(
        "All prerequisites met for deadline {} ({:?})",
        deadline.id, deadline.kind
    );
    Ok(true)
}

/// Transition a deadline to a new status with optional error message
#[instrument]
pub async fn transition_deadline_status<C>(
    deadline_id: i64,
    new_status: DeadlineStatus,
    error_message: Option<String>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    debug!(
        "Transitioning deadline {} to status {:?}",
        deadline_id, new_status
    );

    // Find the deadline
    let deadline = deadline::Entity::find_by_id(deadline_id)
        .one(db)
        .await?
        .ok_or_else(|| color_eyre::eyre::eyre!("Deadline {} not found", deadline_id))?;

    // Create active model for update
    let mut active_deadline: deadline::ActiveModel = deadline.into();
    active_deadline.status = Set(new_status);
    active_deadline.updated_at = Set(Utc::now().into());

    // Set error message if provided and status is Error
    if let Some(error_msg) = error_message {
        if new_status == DeadlineStatus::Error {
            // Note: We'd need to add an error_message column to the deadline table for this
            // For now, we'll log the error and could store it in a separate error log table
            error!("Deadline {} error: {}", deadline_id, error_msg);
        }
    }

    // Save the update
    active_deadline.update(db).await?;

    info!(
        "Successfully transitioned deadline {} to status {:?}",
        deadline_id, new_status
    );
    Ok(())
}
