//! The scheduler half of FBKL's orchestration layer: a DB-driven poll (not cron) that runs
//! inside the long-lived `fbkl-server` process. Each tick discovers due, unprocessed
//! `deadline` rows across **all** leagues and hands them to `fbkl-transaction-processor`,
//! which owns idempotency (`job_run` claims), transactional dispatch, and outcome recording.
//!
//! Retry semantics live in the processor's claim path: a `Failed` `job_run` is reclaimed on a
//! later tick until `MAX_ATTEMPTS`, after which it stays `Failed` and must be retried manually
//! from the commissioner console.
//!
//! Note for replay/backfill (`import-data`): historical replay calls `fbkl_logic` handlers
//! directly and creates no `job_run` rows, so replayed deadlines look unprocessed to this
//! poller. Before enabling the scheduler against a database containing replayed history, mark
//! those deadlines' `job_runs` `Succeeded` (or only seed live-season deadlines).

use std::collections::BTreeMap;

use chrono::Utc;
use color_eyre::eyre::Result;
use fbkl_entity::{
    auction::AuctionKind,
    auction_queries, auction_schedule, auction_schedule_queries, deadline_queries,
    sea_orm::{DatabaseConnection, prelude::DateTimeWithTimeZone},
};
use fbkl_logic::auction::{
    open_scheduled_auction, shorten_open_auctions_for_crunch_window,
    slide_unbid_auctions_down_a_tier,
};
use fbkl_transaction_processor::{
    ProcessOutcome, ProcessableEvent, ProcessableEventKind, process_deadline, process_event,
};
use tokio::task::JoinHandle;
use tracing::{error, info, instrument};

/// How often the scheduler polls for due work.
pub const SCHEDULER_TICK_INTERVAL_SECS: u64 = 30;

/// Counts of what a single scheduler tick did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TickSummary {
    pub processed: usize,
    pub failed: usize,
    pub skipped: usize,
    /// Later deadlines in a league left unattempted because an earlier one didn't succeed.
    pub blocked: usize,
    /// Errors at the orchestration layer itself (claiming/recording), not handler failures.
    pub errors: usize,
}

impl TickSummary {
    const fn merge(&mut self, other: Self) {
        self.processed += other.processed;
        self.failed += other.failed;
        self.skipped += other.skipped;
        self.blocked += other.blocked;
        self.errors += other.errors;
    }
}

/// Runs one scheduler tick: find every due deadline lacking a `Succeeded` `job_run` and process
/// each. Callable directly for tests and the commissioner console's manual trigger.
///
/// Deadlines are processed per league, strictly oldest-first, and a league's chain stops at the
/// first deadline that doesn't reach `Succeeded`. Later deadlines build on earlier ones, so a
/// failed week-2 lock must block week-3 rather than let the scheduler skip ahead into corrupt
/// state. Leagues are independent, so one stuck league never blocks another.
#[instrument(skip(db))]
pub async fn run_scheduler_tick(db: &DatabaseConnection) -> Result<TickSummary> {
    let now = Utc::now().fixed_offset();
    let due_deadlines = deadline_queries::find_due_unprocessed_deadlines(now, db).await?;

    // The query is global oldest-first; bucket per league while preserving that order.
    let mut deadlines_by_league: BTreeMap<i64, Vec<&_>> = BTreeMap::new();
    for deadline_model in &due_deadlines {
        deadlines_by_league
            .entry(deadline_model.league_id)
            .or_default()
            .push(deadline_model);
    }

    let mut summary = TickSummary::default();
    for league_deadlines in deadlines_by_league.values() {
        let mut league_blocked = false;
        for deadline_model in league_deadlines {
            if league_blocked {
                summary.blocked += 1;
                continue;
            }
            match process_deadline(db, deadline_model).await {
                Ok(ProcessOutcome::Processed { .. }) => summary.processed += 1,
                // Already done elsewhere (race) — satisfied, don't block the chain.
                Ok(ProcessOutcome::AlreadyProcessed) => summary.skipped += 1,
                // Not succeeded — block the rest of this league's chain until it does.
                Ok(ProcessOutcome::Failed { .. }) => {
                    summary.failed += 1;
                    league_blocked = true;
                }
                Ok(ProcessOutcome::AlreadyRunning | ProcessOutcome::AttemptsExhausted { .. }) => {
                    summary.skipped += 1;
                    league_blocked = true;
                }
                Err(orchestration_error) => {
                    summary.errors += 1;
                    league_blocked = true;
                    error!(
                        "Scheduler error processing deadline (id = {}): {orchestration_error:?}",
                        deadline_model.id
                    );
                }
            }
        }
    }

    // Slide first: it is an unbid auction's only clock, so closing first expires it (rules §6.3.4).
    summary.merge(run_veteran_auction_release_tick(db, now).await?);
    summary.merge(run_auction_close_tick(db, now).await?);

    // TODO(fbkl-rust-1dk, spec 03): synthesize RFA 48h raise/match window expiries.

    if summary != TickSummary::default() {
        info!(
            "Scheduler tick: {} processed, {} failed, {} skipped, {} blocked, {} errors",
            summary.processed, summary.failed, summary.skipped, summary.blocked, summary.errors
        );
    }

    Ok(summary)
}

/// Closes every auction whose `close_at` has passed (rules §6.4.4 / §8.3.1-.2).
///
/// Runs on every tick, after the release/slide tick so the tier ladder gets to move an unbid
/// auction's clock first. Each close goes through `process_event`, so the `job_run` claim is the
/// double-fire guard.
#[instrument(skip(db))]
pub async fn run_auction_close_tick(
    db: &DatabaseConnection,
    now: DateTimeWithTimeZone,
) -> Result<TickSummary> {
    let mut summary = TickSummary::default();
    for (auction_model, contract_model) in
        auction_queries::find_auctions_due_for_close(now, db).await?
    {
        let event = ProcessableEvent {
            league_id: contract_model.league_id,
            end_of_season_year: contract_model.end_of_season_year,
            auction_id: auction_model.id,
            kind: match auction_model.kind {
                AuctionKind::InSeasonFreeAgent => ProcessableEventKind::FaAuctionClose,
                AuctionKind::PreseasonVeteranAuction => ProcessableEventKind::VeteranAuctionClose,
            },
        };
        match process_event(db, event).await {
            Ok(ProcessOutcome::Processed { .. }) => summary.processed += 1,
            Ok(ProcessOutcome::Failed { .. }) => summary.failed += 1,
            Ok(
                ProcessOutcome::AlreadyProcessed
                | ProcessOutcome::AlreadyRunning
                | ProcessOutcome::AttemptsExhausted { .. },
            ) => summary.skipped += 1,
            Err(orchestration_error) => {
                summary.errors += 1;
                error!(
                    "Scheduler error closing auction (id = {}): {orchestration_error:?}",
                    auction_model.id
                );
            }
        }
    }
    Ok(summary)
}

/// Releases the veteran auction players due today, slides unbid auctions a tier (rules §6.3.3-.5),
/// and shortens the reprieve of auctions still live inside the crunch window (§6.4.4).
///
/// Every step is idempotent by construction rather than `job_run`-tracked: opening an already-opened
/// schedule row returns the existing auction, the tier slide only touches auctions untouched for a
/// day, and the crunch sweep only ever moves a close time earlier.
#[instrument(skip(db))]
pub async fn run_veteran_auction_release_tick(
    db: &DatabaseConnection,
    now: DateTimeWithTimeZone,
) -> Result<TickSummary> {
    let mut schedule_rows_by_league_season: BTreeMap<(i64, i16), Vec<_>> = BTreeMap::new();
    for schedule_row in
        auction_schedule_queries::find_auction_schedule_rows_due_for_release(now.date_naive(), db)
            .await?
    {
        schedule_rows_by_league_season
            .entry((schedule_row.league_id, schedule_row.end_of_season_year))
            .or_default()
            .push(schedule_row);
    }
    // Leagues whose whole pool is already released still need the daily tier slide.
    for league_season in auction_queries::find_league_seasons_with_open_auctions(
        AuctionKind::PreseasonVeteranAuction,
        db,
    )
    .await?
    {
        schedule_rows_by_league_season
            .entry(league_season)
            .or_default();
    }

    let mut summary = TickSummary::default();
    for ((league_id, end_of_season_year), schedule_rows) in schedule_rows_by_league_season {
        match run_preseason_auction_tick(db, &schedule_rows, league_id, end_of_season_year, now)
            .await
        {
            Ok(()) => summary.processed += 1,
            Err(release_error) => {
                summary.errors += 1;
                error!(
                    "Veteran auction release tick failed for league {league_id} season {end_of_season_year}: {release_error:?}"
                );
            }
        }
    }
    Ok(summary)
}

async fn run_preseason_auction_tick(
    db: &DatabaseConnection,
    schedule_rows: &[auction_schedule::Model],
    league_id: i64,
    end_of_season_year: i16,
    now: DateTimeWithTimeZone,
) -> Result<()> {
    for schedule_row in schedule_rows {
        open_scheduled_auction(schedule_row, now, db).await?;
    }
    slide_unbid_auctions_down_a_tier(league_id, end_of_season_year, now, db).await?;
    shorten_open_auctions_for_crunch_window(
        league_id,
        end_of_season_year,
        AuctionKind::PreseasonVeteranAuction,
        now,
        db,
    )
    .await?;
    Ok(())
}

/// Spawns the scheduler loop on the tokio runtime. Tick errors are logged, never fatal —
/// the loop runs until the returned handle is aborted (server shutdown).
pub fn spawn_scheduler(db: DatabaseConnection) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(SCHEDULER_TICK_INTERVAL_SECS));
        // If a tick runs long, don't burst to catch up — just resume the cadence.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        info!(
            "Deadline scheduler started (every {}s)",
            SCHEDULER_TICK_INTERVAL_SECS
        );
        loop {
            interval.tick().await;
            if let Err(tick_error) = run_scheduler_tick(&db).await {
                error!("Scheduler tick failed: {tick_error:?}");
            }
        }
    })
}
