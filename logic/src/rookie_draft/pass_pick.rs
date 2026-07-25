//! Passing a rookie-draft pick (§7.3.1).
//!
//! Passing asks nothing of the owner — no roster room, no eligible player — so the only guards are
//! that the row is still unresolved and still on the clock. Unlike every other state change this
//! records a transaction and NO `team_update`: no asset changed hands.
//!
//! The passed row stays in the slate as `Skipped`, so it keeps consuming its `order` slot and the
//! draft moves on to the next `Unused` row rather than snaking back — same as the importer.

use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    deadline::DeadlineKind,
    deadline_queries, draft_pick_queries,
    rookie_draft_selection::{self, RookieDraftSelectionStatus},
    rookie_draft_selection_queries,
    sea_orm::{ConnectionTrait, TransactionTrait},
    transaction_queries,
};
use tracing::instrument;

use super::make_pick::{PickRejection, assert_on_the_clock};

/// Passes the on-the-clock selection (§7.3.1).
#[instrument(skip(db))]
pub async fn pass_pick<C>(selection_id: i64, db: &C) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let db_txn = db.begin().await?;

    // Locking the slate row first is what serializes two clients racing the same pick.
    let selection_model =
        rookie_draft_selection_queries::find_selection_by_id_for_update(selection_id, &db_txn)
            .await?;
    if selection_model.status != RookieDraftSelectionStatus::Unused {
        return Err(PickRejection::SelectionAlreadyResolved { selection_id }.into());
    }

    let draft_pick_model =
        draft_pick_queries::find_draft_pick_by_id(selection_model.draft_pick_id, &db_txn).await?;
    let league_id = selection_model.league_id;
    let end_of_season_year = draft_pick_model.end_of_season_year;

    assert_on_the_clock(&selection_model, end_of_season_year, &db_txn).await?;

    let deadline_model = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::PreseasonRookieDraftStart,
        &db_txn,
    )
    .await?;

    let mut updated_selection_model = rookie_draft_selection_queries::record_selection_result(
        selection_model,
        RookieDraftSelectionStatus::Skipped,
        None,
        &db_txn,
    )
    .await?;

    let transaction_model = transaction_queries::insert_rookie_draft_selection_transaction(
        &deadline_model,
        updated_selection_model.id,
        &db_txn,
    )
    .await?;
    updated_selection_model.transaction_id = Some(transaction_model.id);

    db_txn.commit().await?;

    Ok(updated_selection_model)
}
