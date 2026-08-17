//! Which Rookie-Draft picks a bidder may forfeit when the original owner declines (rules §15.2).
//!
//! The tier table turns the bid into the worst round that will settle the debt; everything else
//! here is subtraction. Rules §15.3.3 bars a bid the bidder could not pay for, so every bid on a
//! restricted free agent names its pick as it is placed and the choice is checked there and then.
//!
//! Naming the pick with the bid also settles rules §15.2.2 without a rule of its own: a pick
//! acquired after the winning bid cannot have been named by it, because the bidder did not hold it
//! at the time. A pick another live bid or handshake has named is out too, because one pick cannot
//! settle two debts.

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use fbkl_entity::{
    draft_pick, draft_pick_queries, rfa_compensation_pick,
    rfa_resolution::{self, RfaResolutionStatus},
    rfa_resolution_queries::{self, NewRfaCompensationPick},
    sea_orm::ConnectionTrait,
};
use tracing::instrument;

/// The picks a bidder may name for a debt of `required_round`, best round first.
///
/// A pick qualifies when the bidder holds it now, its round is no worse than the tier the bid earns
/// (an earlier round always settles a later one, rules §15.2.1), and it is not already named on
/// another of the season's restricted free agents. `excluded_rfa_resolution_id` is the debt being
/// priced, whose own pick is the one about to be replaced.
///
/// An empty result means the bidder cannot pay the tier, which rules §15.3.3 forbids bidding into.
#[instrument(skip(db))]
pub async fn eligible_compensation_picks<C>(
    league_id: i64,
    end_of_season_year: i16,
    team_id: i64,
    required_round: i16,
    excluded_rfa_resolution_id: i64,
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let reserved_pick_ids = rfa_resolution_queries::find_reserved_compensation_pick_ids(
        league_id,
        end_of_season_year,
        team_id,
        excluded_rfa_resolution_id,
        db,
    )
    .await?;
    let season_picks =
        draft_pick_queries::get_draft_picks_for_league_season(league_id, end_of_season_year, db)
            .await?;

    Ok(season_picks
        .into_iter()
        .filter(|season_pick| {
            season_pick.current_owner_team_id == team_id
                && season_pick.round <= required_round
                && !reserved_pick_ids.contains(&season_pick.id)
        })
        .collect())
}

/// The named pick, when it is one the team may forfeit for a debt of `required_round`.
///
/// `None` is a bid rules §15.3.3 refuses: either the pick is too late a round, already named on
/// another restricted free agent, or not the team's to give.
#[instrument(skip(db))]
pub async fn find_eligible_compensation_pick<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    team_id: i64,
    required_round: i16,
    draft_pick_id: i64,
    db: &C,
) -> Result<Option<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let eligible_draft_picks = eligible_compensation_picks(
        rfa_resolution_model.league_id,
        rfa_resolution_model.end_of_season_year,
        team_id,
        required_round,
        rfa_resolution_model.id,
        db,
    )
    .await?;
    Ok(eligible_draft_picks
        .into_iter()
        .find(|eligible_pick| eligible_pick.id == draft_pick_id))
}

/// Records what a bid or raise would forfeit. The row describes the team currently leading, so each
/// call overwrites the last one.
///
/// The pick comes from [`find_eligible_compensation_pick`], which is where rules §15.3.3 is
/// enforced; taking the row rather than an id is what keeps an unchecked pick out of the table.
#[instrument(skip(db))]
pub async fn name_compensation_pick<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    naming_team_id: i64,
    required_round: i16,
    eligible_draft_pick: &draft_pick::Model,
    db: &C,
) -> Result<rfa_compensation_pick::Model>
where
    C: ConnectionTrait,
{
    rfa_resolution_queries::upsert_rfa_compensation_pick(
        NewRfaCompensationPick {
            rfa_resolution_id: rfa_resolution_model.id,
            required_round,
            forfeited_draft_pick_id: eligible_draft_pick.id,
            to_team_id: rfa_resolution_model.original_owner_team_id,
            from_team_id: naming_team_id,
        },
        db,
    )
    .await
}

/// The leading bidder swaps his named pick for another of the same tier or better (rules §15.2.2).
///
/// Rules §15.2.2 lets him choose which eligible pick he gives up, and holding him to one choice
/// made at bid time could block a later bid his remaining picks could otherwise cover. Once the
/// original owner's window opens he is deciding against a named pick, so the choice is fixed there.
#[instrument(skip(db))]
pub async fn change_compensation_pick<C>(
    rfa_resolution_id: i64,
    naming_team_id: i64,
    draft_pick_id: i64,
    db: &C,
) -> Result<rfa_compensation_pick::Model>
where
    C: ConnectionTrait,
{
    let rfa_resolution_model =
        rfa_resolution_queries::find_rfa_resolution_by_id(rfa_resolution_id, db).await?;
    ensure!(
        matches!(
            rfa_resolution_model.status,
            RfaResolutionStatus::AwaitingAuction | RfaResolutionStatus::AwaitingRaise
        ),
        "The compensation pick for RFA resolution {rfa_resolution_id} is already fixed (status: {:?}).",
        rfa_resolution_model.status
    );

    let rfa_compensation_pick_model =
        rfa_resolution_queries::find_rfa_compensation_pick_for_resolution(rfa_resolution_id, db)
            .await?
            .ok_or_else(|| {
                eyre!("Nobody has bid on RFA resolution {rfa_resolution_id}, so no pick is owed.")
            })?;
    ensure!(
        rfa_compensation_pick_model.from_team_id == naming_team_id,
        "Only the leading bidder may name the compensation pick for RFA resolution {rfa_resolution_id}."
    );

    let required_round = rfa_compensation_pick_model.required_round;
    let eligible_draft_pick = find_eligible_compensation_pick(
        &rfa_resolution_model,
        naming_team_id,
        required_round,
        draft_pick_id,
        db,
    )
    .await?
    .ok_or_else(|| {
        eyre!("Draft pick {draft_pick_id} cannot settle RFA resolution {rfa_resolution_id}.")
    })?;

    name_compensation_pick(
        &rfa_resolution_model,
        naming_team_id,
        required_round,
        &eligible_draft_pick,
        db,
    )
    .await
}
