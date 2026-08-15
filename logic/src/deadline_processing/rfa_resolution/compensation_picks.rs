//! Which Rookie-Draft picks a winning owner may forfeit when the original owner declines (rules §15.2).
//!
//! The tier table turns the bid into the worst round that will settle the debt; everything else here
//! is subtraction. The winner chooses from what is left, so this returns the whole set rather than
//! picking one.

use color_eyre::{Result, eyre::eyre};
use fbkl_constants::league_rules::compensation_round_for_bid;
use fbkl_entity::{draft_pick, draft_pick_queries, rfa_resolution, sea_orm::ConnectionTrait};
use tracing::instrument;

/// The picks the winning owner may hand to the original owner, best round first.
///
/// A pick qualifies when the winner still holds it, its round is no worse than the tier the bid
/// earns (an earlier round always settles a later one, rules §15.2.1), and the winner did not
/// acquire it after the winning bid was announced (rules §15.2.2).
///
/// An empty result means the winner owes a pick he cannot pay, which rules §15.3.3 is meant to
/// prevent at bid and raise time.
#[instrument(skip(db))]
pub async fn compute_eligible_compensation_picks<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let resolution_id = rfa_resolution_model.id;
    let winning_team_id = rfa_resolution_model.winning_team_id.ok_or_else(|| {
        eyre!("Nobody won RFA resolution {resolution_id}, so no pick is owed for it.")
    })?;
    let effective_bid = rfa_resolution_model
        .effective_bid()
        .ok_or_else(|| eyre!("RFA resolution {resolution_id} has no bid to price a pick from."))?;
    let final_bid_at = rfa_resolution_model.final_bid_at.ok_or_else(|| {
        eyre!("RFA resolution {resolution_id} is missing the time its winning bid was announced.")
    })?;
    let required_round = compensation_round_for_bid(effective_bid);

    let picks_acquired_after_bid = draft_pick_queries::find_draft_pick_ids_acquired_by_team_after(
        winning_team_id,
        final_bid_at,
        db,
    )
    .await?;
    let season_picks = draft_pick_queries::get_draft_picks_for_league_season(
        rfa_resolution_model.league_id,
        rfa_resolution_model.end_of_season_year,
        db,
    )
    .await?;

    Ok(season_picks
        .into_iter()
        .filter(|season_pick| {
            season_pick.current_owner_team_id == winning_team_id
                && season_pick.round <= required_round
                && !picks_acquired_after_bid.contains(&season_pick.id)
        })
        .collect())
}
