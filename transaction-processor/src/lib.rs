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

use color_eyre::eyre::{Result, eyre};
use fbkl_entity::{
    deadline::{self, DeadlineKind},
    deadline_queries,
    job_run::JobEventKind,
    job_run_queries::{
        ClaimOutcome, NewJobRun, claim_job_run, deadline_idempotency_key, mark_job_run_failed,
        mark_job_run_succeeded,
    },
    rfa_resolution_queries,
    sea_orm::{ActiveEnum, ConnectionTrait, TransactionSession, TransactionTrait},
};
use fbkl_logic::{
    annual_contract_advancement::advance_league_contracts,
    auction::{assemble_veteran_auction_pool, end_fa_auction, end_veteran_auction},
    deadline_processing::{
        RfaMatchDecision, decline_to_raise, lock_rosters, match_or_decline,
        process_keeper_deadline_transaction,
    },
};
use tracing::{error, info, instrument};

/// A time-triggered event that is *not* backed by a row in the `deadline` table.
///
/// Fire-times for these derive from `auction` / RFA-state rows; the scheduler synthesizes them and the
/// processor dispatches them like deadlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessableEvent {
    pub league_id: i64,
    pub end_of_season_year: i16,
    /// The row the event is about: an `auction` id for the close events, an `rfa_resolution` id for
    /// the two RFA window expiries. `kind` says which table it points to.
    pub subject_id: i64,
    pub kind: ProcessableEventKind,
}

/// Which synthesized sub-event fired; all share `{league_id, end_of_season_year, subject_id}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessableEventKind {
    /// An open FA auction reached 24h with no new bids (§8.3.1).
    FaAuctionClose,
    /// An FA auction's §8.3.2 30-min all-bid extension chain expired.
    FaExtensionExpiry,
    /// A preseason veteran auction reached 24h with no new bids (§6.4.4).
    VeteranAuctionClose,
    /// An RFA winner's 48h raise window expired (§15.3.2, spec 03).
    RfaRaiseWindowExpiry,
    /// An RFA owner's 48h match window expired (§15.3.2, spec 03).
    RfaMatchWindowExpiry,
}

impl ProcessableEventKind {
    const fn event_kind(self) -> JobEventKind {
        match self {
            Self::FaAuctionClose => JobEventKind::FaAuctionClose,
            Self::FaExtensionExpiry => JobEventKind::FaExtensionExpiry,
            Self::VeteranAuctionClose => JobEventKind::VeteranAuctionClose,
            Self::RfaRaiseWindowExpiry => JobEventKind::RfaRaiseWindow,
            Self::RfaMatchWindowExpiry => JobEventKind::RfaMatchWindow,
        }
    }

    /// Which table `ProcessableEvent::subject_id` points to, spelled out in the idempotency key.
    const fn subject_label(self) -> &'static str {
        match self {
            Self::FaAuctionClose | Self::FaExtensionExpiry | Self::VeteranAuctionClose => "auction",
            Self::RfaRaiseWindowExpiry | Self::RfaMatchWindowExpiry => "rfa-resolution",
        }
    }
}

impl ProcessableEvent {
    pub const fn event_kind(&self) -> JobEventKind {
        self.kind.event_kind()
    }

    /// Stable idempotency key: `(league_id, end_of_season_year, kind, subject row)`.
    pub fn idempotency_key(&self) -> String {
        // `to_value()` is the persisted string_value contract, unlike Debug which drifts on rename.
        format!(
            "{}:{}:{}:{}-{}",
            self.league_id,
            self.end_of_season_year,
            self.event_kind().to_value(),
            self.kind.subject_label(),
            self.subject_id
        )
    }
}

/// What happened when the processor was handed a deadline/event.
#[derive(Debug, Clone)]
pub enum ProcessOutcome {
    /// The handler ran and committed; `job_run` is `Succeeded`.
    Processed { job_run_id: i64 },
    /// A `Succeeded` `job_run` already existed — nothing was done.
    AlreadyProcessed,
    /// Another worker currently holds the `Running` claim — nothing was done.
    AlreadyRunning,
    /// The run already failed `MAX_ATTEMPTS` times; surfaced to the console, not retried.
    AttemptsExhausted { job_run_id: i64 },
    /// The handler errored; its transaction rolled back and `job_run` is `Failed`.
    Failed { job_run_id: i64, error: String },
}

/// Processes a single due deadline: claim, dispatch the matching `fbkl_logic` handler inside
/// one DB transaction, and record the outcome.
#[instrument(skip(db))]
pub async fn process_deadline<C>(db: &C, deadline_model: &deadline::Model) -> Result<ProcessOutcome>
where
    C: ConnectionTrait + TransactionTrait,
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
    C: ConnectionTrait + TransactionTrait,
{
    let new_job_run = NewJobRun {
        league_id: event.league_id,
        end_of_season_year: event.end_of_season_year,
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
    C: ConnectionTrait + TransactionTrait,
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
/// effects live elsewhere (auction ticks, rookie draft, freezes) are recorded no-ops: their
/// effects are either implicit (e.g. the §4.2.3 cap bump lives in
/// `deadline::Model::get_salary_cap`) or owned by unbuilt engines (specs 01/02), and recording
/// success keeps the scheduler from retrying them forever.
async fn dispatch_deadline<C>(deadline_model: &deadline::Model, txn: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait,
{
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
        // §6.3.1: this deadline builds the season's release schedule; the tick then opens each row on its date.
        DeadlineKind::PreseasonVeteranAuctionStart => {
            assemble_veteran_auction_pool(
                deadline_model.league_id,
                deadline_model.end_of_season_year,
                txn,
            )
            .await?;
            Ok(())
        }
        // Auction opens/closes/tier slides run off `fbkl_jobs`' per-tick auction discovery, and the §8.2 weekly nomination window is enforced at read time in `open_in_season_fa_auction`.
        DeadlineKind::PreseasonFaAuctionStart
        | DeadlineKind::PreseasonFaAuctionEnd
        | DeadlineKind::Week1FreeAgentAuctionStart
        | DeadlineKind::Week1FreeAgentAuctionEnd => {
            info!(
                "Deadline {:?} (id = {}) recorded; auction opens/closes are driven by the scheduler tick",
                deadline_model.kind, deadline_model.id
            );
            Ok(())
        }
        // Scheduler wiring to `start_rookie_draft` is fbkl-rust-z2c; today only the commissioner mutation starts the draft.
        DeadlineKind::PreseasonRookieDraftStart => {
            info!(
                "Deadline {:?} (id = {}) recorded; rookie draft starts via commissioner mutation",
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
async fn dispatch_event<C>(event: ProcessableEvent, txn: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait,
{
    let ProcessableEvent {
        league_id,
        subject_id,
        kind,
        ..
    } = event;
    let now = chrono::Utc::now().fixed_offset();
    match kind {
        ProcessableEventKind::FaAuctionClose | ProcessableEventKind::FaExtensionExpiry => {
            // The most recently passed deadline supplies the signed contract's effective date.
            let deadline_model =
                deadline_queries::find_most_recent_deadline_by_datetime(league_id, now, txn)
                    .await?;
            end_fa_auction(&deadline_model, subject_id, None, txn).await?;
            Ok(())
        }
        ProcessableEventKind::VeteranAuctionClose => {
            end_veteran_auction(subject_id, None, txn).await?;
            Ok(())
        }
        // §15.3.2.1: no raise inside 48h counts as standing pat, which opens the owner's window.
        ProcessableEventKind::RfaRaiseWindowExpiry => {
            let rfa_resolution_model =
                rfa_resolution_queries::find_rfa_resolution_by_id(subject_id, txn).await?;
            let winning_team_id = rfa_resolution_model.winning_team_id.ok_or_else(|| {
                eyre!("RFA resolution {subject_id} has no winning bidder to stand pat for.")
            })?;
            decline_to_raise(subject_id, winning_team_id, now, txn).await?;
            Ok(())
        }
        // §15.3.2.2: no match inside 48h counts as declining, so the winner signs and forfeits.
        ProcessableEventKind::RfaMatchWindowExpiry => {
            let rfa_resolution_model =
                rfa_resolution_queries::find_rfa_resolution_by_id(subject_id, txn).await?;
            // Nobody named a pick, so the cheapest eligible one goes. Commissioners may confirm.
            match_or_decline(
                subject_id,
                rfa_resolution_model.original_owner_team_id,
                RfaMatchDecision::Decline,
                None,
                now,
                txn,
            )
            .await?;
            Ok(())
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
        let close = ProcessableEvent {
            league_id: 7,
            end_of_season_year: 2027,
            subject_id: 99,
            kind: ProcessableEventKind::FaAuctionClose,
        };
        let extension = ProcessableEvent {
            league_id: 7,
            end_of_season_year: 2027,
            subject_id: 99,
            kind: ProcessableEventKind::FaExtensionExpiry,
        };
        assert_eq!(close.idempotency_key(), "7:2027:FaAuctionClose:auction-99");
        assert_ne!(close.idempotency_key(), extension.idempotency_key());
    }
}
