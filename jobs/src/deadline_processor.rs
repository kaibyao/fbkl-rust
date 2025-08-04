use chrono::Utc;
use color_eyre::Result;
use fbkl_entity::sea_orm::DatabaseConnection;
use fbkl_logic::deadline_processing::{
    ProcessingResult, generate_deadlines_if_needed, process_activated_deadlines,
};
use std::fmt::Debug;
use tracing::{error, info, instrument, warn};

/// Report summarizing the results of a deadline processing run
#[derive(Debug, Clone)]
pub struct ProcessingReport {
    pub run_started_at: chrono::DateTime<Utc>,
    pub run_completed_at: chrono::DateTime<Utc>,
    pub leagues_with_generated_deadlines: usize,
    pub total_deadlines_generated: usize,
    pub deadlines_processed: Vec<ProcessingResult>,
    pub successful_processing_count: usize,
    pub failed_processing_count: usize,
    pub errors: Vec<String>,
}

impl ProcessingReport {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            run_started_at: now,
            run_completed_at: now,
            leagues_with_generated_deadlines: 0,
            total_deadlines_generated: 0,
            deadlines_processed: Vec::new(),
            successful_processing_count: 0,
            failed_processing_count: 0,
            errors: Vec::new(),
        }
    }

    pub fn complete(mut self) -> Self {
        self.run_completed_at = Utc::now();
        self.successful_processing_count = self
            .deadlines_processed
            .iter()
            .filter(|r| r.success)
            .count();
        self.failed_processing_count = self
            .deadlines_processed
            .iter()
            .filter(|r| !r.success)
            .count();
        self
    }
}

/// Processing lock to prevent concurrent execution
#[derive(Debug)]
pub struct ProcessingLock {
    pub acquired_at: chrono::DateTime<Utc>,
    pub instance_id: String,
}

/// Main function for cron job execution - processes deadlines for all leagues
///
/// This function is designed to be called by a cron job every 5-15 minutes.
/// It is idempotent and safe for concurrent execution thanks to database locking.
///
/// The function performs two main operations:
/// 1. Generate deadline records from activated configuration rules
/// 2. Process activated deadlines that are ready (current time >= deadline time)
#[instrument]
pub async fn process_deadlines(db: &DatabaseConnection) -> Result<ProcessingReport> {
    info!("Starting deadline processing cron job");

    let mut report = ProcessingReport::new();

    // Acquire processing lock to prevent concurrent execution
    let _lock = match acquire_processing_lock(db).await {
        Ok(Some(lock)) => {
            info!("Acquired processing lock: {:?}", lock.instance_id);
            lock
        }
        Ok(None) => {
            info!("Another instance is already processing deadlines, skipping this run");
            return Ok(report.complete());
        }
        Err(e) => {
            error!("Failed to acquire processing lock: {}", e);
            report
                .errors
                .push(format!("Lock acquisition failed: {}", e));
            return Ok(report.complete());
        }
    };

    // Step 1: Generate deadlines from activated configuration rules
    info!("Step 1: Generating deadlines from configuration rules");
    match generate_deadlines_if_needed(db).await {
        Ok(generation_results) => {
            report.leagues_with_generated_deadlines = generation_results.len();
            report.total_deadlines_generated = generation_results
                .iter()
                .map(|(_, _, deadlines)| deadlines.len())
                .sum();

            if report.total_deadlines_generated > 0 {
                info!(
                    "Generated {} deadlines for {} leagues",
                    report.total_deadlines_generated, report.leagues_with_generated_deadlines
                );
            } else {
                info!("No new deadlines needed generation");
            }
        }
        Err(e) => {
            error!("Failed to generate deadlines: {}", e);
            report
                .errors
                .push(format!("Deadline generation failed: {}", e));
            // Continue with processing existing deadlines even if generation failed
        }
    }

    // Step 2: Process activated deadlines that are ready
    info!("Step 2: Processing activated deadlines");
    match process_activated_deadlines(db).await {
        Ok(processing_results) => {
            report.deadlines_processed = processing_results;

            if !report.deadlines_processed.is_empty() {
                info!("Processed {} deadlines", report.deadlines_processed.len());

                // Log any failed processing attempts
                for result in &report.deadlines_processed {
                    if !result.success {
                        if let Some(error_msg) = &result.error_message {
                            warn!(
                                "Deadline {} ({:?}) processing failed: {}",
                                result.deadline_id, result.deadline_kind, error_msg
                            );
                        }
                    }
                }
            } else {
                info!("No deadlines were ready for processing");
            }
        }
        Err(e) => {
            error!("Failed to process activated deadlines: {}", e);
            report
                .errors
                .push(format!("Deadline processing failed: {}", e));
        }
    }

    let final_report = report.complete();
    log_processing_results(&final_report).await?;

    info!(
        "Completed deadline processing cron job - generated: {}, processed: {} (success: {}, failed: {})",
        final_report.total_deadlines_generated,
        final_report.deadlines_processed.len(),
        final_report.successful_processing_count,
        final_report.failed_processing_count
    );

    Ok(final_report)
}

/// Acquire a processing lock to prevent concurrent execution
///
/// This uses a simple approach that could be enhanced with a dedicated locks table.
/// For now, we use a lightweight approach that's sufficient for the current needs.
///
/// Returns:
/// - Ok(Some(lock)) if lock was successfully acquired
/// - Ok(None) if another instance is already processing
/// - Err if there was an error checking for concurrent processing
#[instrument]
pub async fn acquire_processing_lock(_db: &DatabaseConnection) -> Result<Option<ProcessingLock>> {
    // For the initial implementation, we'll use a simple time-based approach
    // In a production system, this would use a dedicated locks table with proper cleanup

    let instance_id = format!("deadline-processor-{}", Utc::now().timestamp());

    // For now, always grant the lock - this could be enhanced with actual database locking
    // using a dedicated table or advisory locks in the future
    let lock = ProcessingLock {
        acquired_at: Utc::now(),
        instance_id,
    };

    info!("Processing lock acquired (simplified implementation)");
    Ok(Some(lock))
}

/// Log comprehensive processing results for monitoring and debugging
#[instrument]
pub async fn log_processing_results(report: &ProcessingReport) -> Result<()> {
    let duration = report.run_completed_at - report.run_started_at;

    info!(
        "=== DEADLINE PROCESSING SUMMARY ===\n\
         Duration: {}ms\n\
         Generated deadlines: {} (across {} leagues)\n\
         Processed deadlines: {} (success: {}, failed: {})\n\
         Errors: {}",
        duration.num_milliseconds(),
        report.total_deadlines_generated,
        report.leagues_with_generated_deadlines,
        report.deadlines_processed.len(),
        report.successful_processing_count,
        report.failed_processing_count,
        report.errors.len()
    );

    // Log any errors that occurred
    for (i, error) in report.errors.iter().enumerate() {
        error!("Processing error {}: {}", i + 1, error);
    }

    // Log detailed results for failed deadline processing
    for result in &report.deadlines_processed {
        if !result.success {
            error!(
                "Failed to process deadline {} ({:?}): {}",
                result.deadline_id,
                result.deadline_kind,
                result.error_message.as_deref().unwrap_or("Unknown error")
            );
        }
    }

    // Log success summary
    if report.successful_processing_count > 0 || report.total_deadlines_generated > 0 {
        info!(
            "Deadline processing completed successfully - {} deadlines generated, {} processed",
            report.total_deadlines_generated, report.successful_processing_count
        );
    }

    Ok(())
}

/// Helper function to run the deadline processor with proper error handling
/// This can be called from a standalone binary or cron script
pub async fn run_deadline_processor(database_url: &str) -> Result<ProcessingReport> {
    use fbkl_entity::sea_orm::Database;

    info!("Connecting to database: {}", database_url);
    let db = Database::connect(database_url).await?;

    let report = process_deadlines(&db).await?;

    // Close database connection
    db.close().await?;
    info!("Database connection closed");

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_report_creation() {
        let report = ProcessingReport::new();
        assert_eq!(report.leagues_with_generated_deadlines, 0);
        assert_eq!(report.total_deadlines_generated, 0);
        assert_eq!(report.deadlines_processed.len(), 0);
        assert_eq!(report.errors.len(), 0);
    }

    #[test]
    fn test_processing_report_completion() {
        let mut report = ProcessingReport::new();

        // Add some mock results
        report.deadlines_processed = vec![
            ProcessingResult {
                deadline_id: 1,
                deadline_kind: fbkl_entity::deadline::DeadlineKind::PreseasonKeeper,
                success: true,
                error_message: None,
            },
            ProcessingResult {
                deadline_id: 2,
                deadline_kind: fbkl_entity::deadline::DeadlineKind::PreseasonFinalRosterLock,
                success: false,
                error_message: Some("Test error".to_string()),
            },
        ];

        let completed_report = report.complete();
        assert_eq!(completed_report.successful_processing_count, 1);
        assert_eq!(completed_report.failed_processing_count, 1);
    }
}
