//! The transaction processor is the *dispatcher* of FBKL's orchestration layer: given a due
//! `deadline` row or a synthesized sub-event, it runs the matching `fbkl_logic` handler inside
//! a single DB transaction, idempotently, and records the outcome as a `job_run` row.
//!
//! Discovery of *what* is due lives in the `fbkl-jobs` crate (the scheduler); this crate only
//! processes what it is handed. The commissioner console's manual "process now" / "retry"
//! actions call the same entry points, so manual and automatic processing share the same
//! idempotency guarantees and audit trail.
//!
//! Processing happens in three steps:
//! 1. **Claim** a `job_run` row (unique `idempotency_key` = double-fire guard). The claim is
//!    committed outside the handler's transaction so concurrent ticks can observe `Running`.
//! 2. **Dispatch** the handler inside `db.begin()` … `commit()` — a failure rolls back all of
//!    the handler's writes so a retry starts clean.
//! 3. **Record** the outcome (`Succeeded` / `Failed` + error detail) on the `job_run`.

use std::fmt::Debug;

use color_eyre::eyre::{Result, bail};
use fbkl_entity::{
    deadline::{self, DeadlineKind},
    deadline_queries,
    job_run::JobEventKind,
    job_run_queries::{
        ClaimOutcome, NewJobRun, claim_job_run, mark_job_run_failed, mark_job_run_succeeded,
    },
    sea_orm::{ConnectionTrait, DatabaseTransaction, TransactionTrait},
};
use fbkl_logic::{
    annual_contract_advancement::advance_league_contracts,
    auction::{end_fa_auction, end_veteran_auction},
    deadline_processing::{lock_rosters, process_keeper_deadline_transaction},
};
use tracing::{error, info, instrument};

/// A time-triggered event that is *not* backed by a row in the `deadline` table. Fire-times
/// for these derive from `auction` / RFA-state rows; the scheduler synthesizes them and the
/// processor dispatches them like deadlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessableEvent {
    /// An open FA auction reached 24h with no new bids (§8.3.1).
    FaAuctionClose {
        league_id: i64,
        end_of_season_year: i16,
        auction_id: i64,
    },
    /// An FA auction's §8.3.2 30-min all-bid extension chain expired.
    FaExtensionExpiry {
        league_id: i64,
        end_of_season_year: i16,
        auction_id: i64,
    },
    /// A preseason veteran auction reached 24h with no new bids (§6.4.4).
    VeteranAuctionClose {
        league_id: i64,
        end_of_season_year: i16,
        auction_id: i64,
    },
    /// An RFA winner's 48h raise window expired (§15.3.2, spec 03).
    RfaRaiseWindowExpiry {
        league_id: i64,
        end_of_season_year: i16,
        auction_id: i64,
    },
    /// An RFA owner's 48h match window expired (§15.3.2, spec 03).
    RfaMatchWindowExpiry {
        league_id: i64,
        end_of_season_year: i16,
        auction_id: i64,
    },
}

impl ProcessableEvent {
    pub fn league_id(&self) -> i64 {
        match self {
            Self::FaAuctionClose { league_id, .. }
            | Self::FaExtensionExpiry { league_id, .. }
            | Self::VeteranAuctionClose { league_id, .. }
            | Self::RfaRaiseWindowExpiry { league_id, .. }
            | Self::RfaMatchWindowExpiry { league_id, .. } => *league_id,
        }
    }

    pub fn end_of_season_year(&self) -> i16 {
        match self {
            Self::FaAuctionClose {
                end_of_season_year, ..
            }
            | Self::FaExtensionExpiry {
                end_of_season_year, ..
            }
            | Self::VeteranAuctionClose {
                end_of_season_year, ..
            }
            | Self::RfaRaiseWindowExpiry {
                end_of_season_year, ..
            }
            | Self::RfaMatchWindowExpiry {
                end_of_season_year, ..
            } => *end_of_season_year,
        }
    }

    pub fn auction_id(&self) -> i64 {
        match self {
            Self::FaAuctionClose { auction_id, .. }
            | Self::FaExtensionExpiry { auction_id, .. }
            | Self::VeteranAuctionClose { auction_id, .. }
            | Self::RfaRaiseWindowExpiry { auction_id, .. }
            | Self::RfaMatchWindowExpiry { auction_id, .. } => *auction_id,
        }
    }

    pub fn event_kind(&self) -> JobEventKind {
        match self {
            Self::FaAuctionClose { .. } => JobEventKind::FaAuctionClose,
            Self::FaExtensionExpiry { .. } => JobEventKind::FaExtensionExpiry,
            Self::VeteranAuctionClose { .. } => JobEventKind::VeteranAuctionClose,
            Self::RfaRaiseWindowExpiry { .. } => JobEventKind::RfaRaiseWindow,
            Self::RfaMatchWindowExpiry { .. } => JobEventKind::RfaMatchWindow,
        }
    }

    /// Stable idempotency key: `(league_id, end_of_season_year, kind, auction_id)`.
    pub fn idempotency_key(&self) -> String {
        format!(
            "{}:{}:{:?}:auction-{}",
            self.league_id(),
            self.end_of_season_year(),
            self.event_kind(),
            self.auction_id()
        )
    }
}

/// What happened when the processor was handed a deadline/event.
#[derive(Debug, Clone)]
pub enum ProcessOutcome {
    /// The handler ran and committed; `job_run` is `Succeeded`.
    Processed { job_run_id: i64 },
    /// A `Succeeded` job_run already existed — nothing was done.
    AlreadyProcessed,
    /// Another worker currently holds the `Running` claim — nothing was done.
    AlreadyRunning,
    /// The run already failed `MAX_ATTEMPTS` times; surfaced to the console, not retried.
    AttemptsExhausted { job_run_id: i64 },
    /// The handler errored; its transaction rolled back and `job_run` is `Failed`.
    Failed { job_run_id: i64, error: String },
}

/// Stable idempotency key for a deadline row: `(league_id, end_of_season_year, kind, id)`.
/// Weekly locks share a kind across distinct rows, so the deadline id disambiguates.
pub fn deadline_idempotency_key(deadline_model: &deadline::Model) -> String {
    format!(
        "{}:{}:{:?}:deadline-{}",
        deadline_model.league_id,
        deadline_model.end_of_season_year,
        deadline_model.kind,
        deadline_model.id
    )
}

/// Processes a single due deadline: claim, dispatch the matching `fbkl_logic` handler inside
/// one DB transaction, and record the outcome.
#[instrument(skip(db))]
pub async fn process_deadline<C>(db: &C, deadline_model: &deadline::Model) -> Result<ProcessOutcome>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let new_job_run = NewJobRun {
        league_id: deadline_model.league_id,
        end_of_season_year: deadline_model.end_of_season_year,
        deadline_id: Some(deadline_model.id),
        event_kind: JobEventKind::Deadline,
        dispatch_target: format!("{:?}", deadline_model.kind),
        idempotency_key: deadline_idempotency_key(deadline_model),
    };

    run_claimed(db, new_job_run, DispatchTask::Deadline(deadline_model)).await
}

/// Processes a single synthesized sub-event (auction close, RFA window expiry).
#[instrument(skip(db))]
pub async fn process_event<C>(db: &C, event: ProcessableEvent) -> Result<ProcessOutcome>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let new_job_run = NewJobRun {
        league_id: event.league_id(),
        end_of_season_year: event.end_of_season_year(),
        deadline_id: None,
        event_kind: event.event_kind(),
        dispatch_target: format!("{:?}", event.event_kind()),
        idempotency_key: event.idempotency_key(),
    };

    run_claimed(db, new_job_run, DispatchTask::Event(event)).await
}

/// The unit of work to run inside the claimed job's DB transaction.
#[derive(Debug, Clone, Copy)]
enum DispatchTask<'m> {
    Deadline(&'m deadline::Model),
    Event(ProcessableEvent),
}

/// Shared claim → dispatch-in-transaction → record-outcome flow for deadlines and sub-events.
async fn run_claimed<C>(
    db: &C,
    new_job_run: NewJobRun,
    task: DispatchTask<'_>,
) -> Result<ProcessOutcome>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let dispatch_target = new_job_run.dispatch_target.clone();
    let job_run_model = match claim_job_run(new_job_run, db).await? {
        ClaimOutcome::Claimed(job_run_model) => job_run_model,
        ClaimOutcome::AlreadySucceeded => return Ok(ProcessOutcome::AlreadyProcessed),
        ClaimOutcome::AlreadyRunning => return Ok(ProcessOutcome::AlreadyRunning),
        ClaimOutcome::AttemptsExhausted(job_run_model) => {
            return Ok(ProcessOutcome::AttemptsExhausted {
                job_run_id: job_run_model.id,
            });
        }
    };

    let txn = db.begin().await?;
    let dispatch_result = match task {
        DispatchTask::Deadline(deadline_model) => dispatch_deadline(deadline_model, &txn).await,
        DispatchTask::Event(event) => dispatch_event(event, &txn).await,
    };
    match dispatch_result {
        Ok(()) => {
            // Record success inside the handler's transaction so the handler's effects and the
            // Succeeded job_run commit atomically — otherwise a crash between commit and record
            // would leave committed work behind a still-Running job_run and invite a re-run.
            mark_job_run_succeeded(job_run_model.id, None, &txn).await?;
            txn.commit().await?;
            info!(
                "Processed {dispatch_target} (job_run id = {})",
                job_run_model.id
            );
            Ok(ProcessOutcome::Processed {
                job_run_id: job_run_model.id,
            })
        }
        Err(handler_error) => {
            txn.rollback().await?;
            let error_detail = format!("{handler_error:?}");
            mark_job_run_failed(job_run_model.id, &error_detail, db).await?;
            error!(
                "Failed processing {dispatch_target} (job_run id = {}, attempt {}): {error_detail}",
                job_run_model.id, job_run_model.attempts
            );
            Ok(ProcessOutcome::Failed {
                job_run_id: job_run_model.id,
                error: error_detail,
            })
        }
    }
}

/// Maps a deadline kind to its `fbkl_logic` handler (the spec-05 dispatch table). Kinds whose
/// engines are not yet built (auction opens, rookie draft, freezes) are recorded no-ops: their
/// effects are either implicit (e.g. the §4.2.3 cap bump lives in
/// `deadline::Model::get_salary_cap`) or owned by unbuilt engines (specs 01/02), and recording
/// success keeps the scheduler from retrying them forever.
async fn dispatch_deadline(
    deadline_model: &deadline::Model,
    txn: &DatabaseTransaction,
) -> Result<()> {
    match deadline_model.kind {
        DeadlineKind::PreseasonStart => {
            advance_league_contracts(
                deadline_model.league_id,
                deadline_model.end_of_season_year,
                txn,
            )
            .await?;
            Ok(())
        }
        DeadlineKind::PreseasonKeeper => {
            process_keeper_deadline_transaction(
                deadline_model.league_id,
                deadline_model.end_of_season_year,
                txn,
            )
            .await
        }
        DeadlineKind::PreseasonFinalRosterLock
        | DeadlineKind::Week1RosterLock
        | DeadlineKind::InSeasonRosterLock => lock_rosters(deadline_model, txn).await,
        // Auction engine lifecycle deadlines — spec 01 (fbkl-rust-lcc) owns opening/closing
        // auction state; until then the deadline passing is itself the only effect.
        DeadlineKind::PreseasonVeteranAuctionStart
        | DeadlineKind::PreseasonFaAuctionStart
        | DeadlineKind::PreseasonFaAuctionEnd
        | DeadlineKind::Week1FreeAgentAuctionStart
        | DeadlineKind::Week1FreeAgentAuctionEnd => {
            info!(
                "Deadline {:?} (id = {}) recorded; auction engine lifecycle is spec 01",
                deadline_model.kind, deadline_model.id
            );
            Ok(())
        }
        // Rookie draft engine is spec 02 (fbkl-rust-2jq).
        DeadlineKind::PreseasonRookieDraftStart => {
            info!(
                "Deadline {:?} (id = {}) recorded; rookie draft engine is spec 02",
                deadline_model.kind, deadline_model.id
            );
            Ok(())
        }
        // The §4.2.3 $20 cap bump and §4.2.4 cap removal are resolved at read time by
        // `deadline::Model::get_salary_cap`; the trade freeze (§12.3) is enforced at
        // trade-processing time. Recording success marks the period transition as observed.
        DeadlineKind::FreeAgentAuctionEnd
        | DeadlineKind::TradeDeadlineAndPlayoffStart
        | DeadlineKind::SeasonEnd => {
            info!(
                "Deadline {:?} (id = {}) recorded; period transition is resolved at read time",
                deadline_model.kind, deadline_model.id
            );
            Ok(())
        }
    }
}

/// Maps a sub-event to its `fbkl_logic` handler.
async fn dispatch_event(event: ProcessableEvent, txn: &DatabaseTransaction) -> Result<()> {
    match event {
        ProcessableEvent::FaAuctionClose {
            league_id,
            auction_id,
            ..
        }
        | ProcessableEvent::FaExtensionExpiry {
            league_id,
            auction_id,
            ..
        } => {
            // The deadline supplies the effective date for the signed contract — use the most
            // recently passed deadline at close time.
            let deadline_model = deadline_queries::find_most_recent_deadline_by_datetime(
                league_id,
                chrono::Utc::now().fixed_offset(),
                txn,
            )
            .await?;
            end_fa_auction(&deadline_model, auction_id, None, txn).await?;
            Ok(())
        }
        ProcessableEvent::VeteranAuctionClose { auction_id, .. } => {
            end_veteran_auction(auction_id, None, txn).await?;
            Ok(())
        }
        ProcessableEvent::RfaRaiseWindowExpiry { auction_id, .. }
        | ProcessableEvent::RfaMatchWindowExpiry { auction_id, .. } => {
            // RFA resolution is spec 03 (fbkl-rust-1dk). Bail (terminal failure) rather than
            // silently succeed: these events should not be synthesized until that engine lands.
            bail!(
                "RFA window processing is not implemented (spec 03); auction_id = {}",
                auction_id
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fbkl_entity::sea_orm::prelude::DateTimeWithTimeZone;

    fn test_deadline(kind: DeadlineKind) -> deadline::Model {
        let datetime = DateTimeWithTimeZone::parse_from_rfc3339("2026-10-01T00:00:00Z").unwrap();
        deadline::Model {
            id: 42,
            date_time: datetime,
            kind,
            name: "Test deadline".to_string(),
            end_of_season_year: 2027,
            league_id: 7,
            created_at: datetime,
            updated_at: datetime,
        }
    }

    #[test]
    fn deadline_idempotency_key_is_stable_and_distinct_per_deadline_row() {
        let deadline_model = test_deadline(DeadlineKind::InSeasonRosterLock);
        assert_eq!(
            deadline_idempotency_key(&deadline_model),
            "7:2027:InSeasonRosterLock:deadline-42"
        );

        // Weekly locks share a kind; the deadline id keeps their keys distinct.
        let mut other_week = test_deadline(DeadlineKind::InSeasonRosterLock);
        other_week.id = 43;
        assert_ne!(
            deadline_idempotency_key(&deadline_model),
            deadline_idempotency_key(&other_week)
        );
    }

    #[test]
    fn event_idempotency_key_distinguishes_event_kinds_for_same_auction() {
        let close = ProcessableEvent::FaAuctionClose {
            league_id: 7,
            end_of_season_year: 2027,
            auction_id: 99,
        };
        let extension = ProcessableEvent::FaExtensionExpiry {
            league_id: 7,
            end_of_season_year: 2027,
            auction_id: 99,
        };
        assert_eq!(close.idempotency_key(), "7:2027:FaAuctionClose:auction-99");
        assert_ne!(close.idempotency_key(), extension.idempotency_key());
    }
}
