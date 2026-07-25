//! Starting the live rookie draft (§7.2).
//!
//! The scheduler runs this at the `PreseasonRookieDraftStart` deadline, and a commissioner can run
//! it manually if the scheduler missed. Both paths reach the same function, so every step is
//! idempotent: an existing slate short-circuits, and [`run_lottery`] replays a stored draw instead
//! of re-rolling it.

use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    deadline::DeadlineKind,
    deadline_queries, draft_pick_queries, league_team_season_standing_queries,
    rookie_draft_selection_queries,
    sea_orm::{ConnectionTrait, TransactionTrait},
};
use tracing::instrument;

use super::{compute_draft_order, run_lottery};

/// Runs the lottery and persists the full ordered slate of unused selections (§7.2).
///
/// Returns `false` when the draft had already been started and nothing was written.
#[instrument]
pub async fn start_rookie_draft<C>(league_id: i64, end_of_season_year: i16, db: &C) -> Result<bool>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    // Errors when the deadline is missing, so a league without a scheduled draft cannot start one.
    deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::PreseasonRookieDraftStart,
        db,
    )
    .await?;

    if !rookie_draft_selection_queries::get_selections_for_draft(league_id, end_of_season_year, db)
        .await?
        .is_empty()
    {
        return Ok(false);
    }

    let db_txn = db.begin().await?;

    let standings = league_team_season_standing_queries::find_standings_for_league_season(
        league_id,
        end_of_season_year,
        &db_txn,
    )
    .await?;
    let lottery_team_order =
        run_lottery(league_id, end_of_season_year, &standings, &db_txn).await?;
    let draft_picks = draft_pick_queries::get_draft_picks_for_league_season(
        league_id,
        end_of_season_year,
        &db_txn,
    )
    .await?;

    let draft_slots = compute_draft_order(&standings, &lottery_team_order, &draft_picks)?;
    rookie_draft_selection_queries::build_draft_slate(
        league_id,
        end_of_season_year,
        draft_slots.iter().map(|slot| slot.draft_pick_id).collect(),
        &db_txn,
    )
    .await?;

    db_txn.commit().await?;

    Ok(true)
}
