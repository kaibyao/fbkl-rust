//! Reads/writes for the RFA raise/match handshake rows (rules §15.2, §15.3).
//!
//! Every rule about who may raise, match or decline lives in `logic`; these are plain row
//! accessors. The scheduler drives the two 48h windows off
//! [`find_rfa_resolutions_with_expired_window`].

use std::fmt::Debug;

use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectionTrait, EntityTrait,
    QueryFilter, QueryOrder, prelude::DateTimeWithTimeZone,
};
use tracing::instrument;

use crate::{
    contract_queries, rfa_compensation_pick,
    rfa_resolution::{self, RfaResolutionStatus},
};

/// A resolution to seed. Designation writes one with only the first four fields filled in; the
/// auction fills in the rest when it closes.
#[derive(Clone, Copy, Debug)]
pub struct NewRfaResolution {
    pub league_id: i64,
    pub end_of_season_year: i16,
    pub rfa_contract_id: i64,
    pub original_owner_team_id: i64,
    /// NULL for a player nobody bid on (rules §15.3.5).
    pub auction_id: Option<i64>,
    pub winning_team_id: Option<i64>,
    pub final_bid: Option<i16>,
    pub final_bid_at: Option<DateTimeWithTimeZone>,
    pub status: RfaResolutionStatus,
    /// NULL until the auction closes (rules §15.3.2.1).
    pub raise_deadline_at: Option<DateTimeWithTimeZone>,
}

#[instrument(skip(db))]
pub async fn insert_rfa_resolution<C>(
    new_rfa_resolution: NewRfaResolution,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    let rfa_resolution_to_insert = rfa_resolution::ActiveModel {
        id: ActiveValue::NotSet,
        league_id: ActiveValue::Set(new_rfa_resolution.league_id),
        end_of_season_year: ActiveValue::Set(new_rfa_resolution.end_of_season_year),
        rfa_contract_id: ActiveValue::Set(new_rfa_resolution.rfa_contract_id),
        original_owner_team_id: ActiveValue::Set(new_rfa_resolution.original_owner_team_id),
        auction_id: ActiveValue::Set(new_rfa_resolution.auction_id),
        winning_team_id: ActiveValue::Set(new_rfa_resolution.winning_team_id),
        final_bid: ActiveValue::Set(new_rfa_resolution.final_bid),
        final_bid_at: ActiveValue::Set(new_rfa_resolution.final_bid_at),
        status: ActiveValue::Set(new_rfa_resolution.status),
        raised_bid: ActiveValue::NotSet,
        raise_deadline_at: ActiveValue::Set(new_rfa_resolution.raise_deadline_at),
        match_deadline_at: ActiveValue::NotSet,
        resolved_at: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };
    Ok(rfa_resolution_to_insert.insert(db).await?)
}

#[instrument(skip(db))]
pub async fn find_rfa_resolution_by_id<C>(
    rfa_resolution_id: i64,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    rfa_resolution::Entity::find_by_id(rfa_resolution_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find RFA resolution with id: {rfa_resolution_id}"))
}

/// The resolution for a designated RFA contract, if designation has run for it.
///
/// Matches any contract in the same season's chain, because a trade between the keeper deadline and
/// the auction replaces the contract row and the resolution still points at the older id.
#[instrument(skip(db))]
pub async fn find_rfa_resolution_for_contract<C>(
    rfa_contract_id: i64,
    db: &C,
) -> Result<Option<rfa_resolution::Model>>
where
    C: ConnectionTrait,
{
    let given_contract = contract_queries::find_contract_by_id(rfa_contract_id, db).await?;
    let season_chain_ids: Vec<i64> = contract_queries::find_contract_chain(rfa_contract_id, db)
        .await?
        .into_iter()
        .filter_map(|chain_contract| {
            (chain_contract.end_of_season_year == given_contract.end_of_season_year)
                .then_some(chain_contract.id)
        })
        .collect();

    let maybe_rfa_resolution = rfa_resolution::Entity::find()
        .filter(rfa_resolution::Column::RfaContractId.is_in(season_chain_ids))
        .one(db)
        .await?;
    Ok(maybe_rfa_resolution)
}

/// Every resolution in a league season, oldest first.
#[instrument(skip(db))]
pub async fn find_rfa_resolutions_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<rfa_resolution::Model>>
where
    C: ConnectionTrait,
{
    let rfa_resolutions = rfa_resolution::Entity::find()
        .filter(rfa_resolution::Column::LeagueId.eq(league_id))
        .filter(rfa_resolution::Column::EndOfSeasonYear.eq(end_of_season_year))
        .order_by_asc(rfa_resolution::Column::Id)
        .all(db)
        .await?;
    Ok(rfa_resolutions)
}

/// Resolutions whose open window has run out by `now` — the scheduler's work list for both 48h
/// timeouts. The row's own status says which window it is.
#[instrument(skip(db))]
pub async fn find_rfa_resolutions_with_expired_window<C>(
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<Vec<rfa_resolution::Model>>
where
    C: ConnectionTrait,
{
    let expired_rfa_resolutions = rfa_resolution::Entity::find()
        .filter(
            Condition::any()
                .add(
                    Condition::all()
                        .add(rfa_resolution::Column::Status.eq(RfaResolutionStatus::AwaitingRaise))
                        .add(rfa_resolution::Column::RaiseDeadlineAt.lte(now)),
                )
                .add(
                    Condition::all()
                        .add(rfa_resolution::Column::Status.eq(RfaResolutionStatus::AwaitingMatch))
                        .add(rfa_resolution::Column::MatchDeadlineAt.lte(now)),
                ),
        )
        .order_by_asc(rfa_resolution::Column::Id)
        .all(db)
        .await?;
    Ok(expired_rfa_resolutions)
}

/// Everything a closed RFA auction hands to its resolution.
#[derive(Clone, Copy, Debug)]
pub struct ClosedRfaAuctionResult {
    pub auction_id: i64,
    pub winning_team_id: i64,
    pub final_bid: i16,
    pub final_bid_at: DateTimeWithTimeZone,
    /// Auction close + 48h (rules §15.3.2.1).
    pub raise_deadline_at: DateTimeWithTimeZone,
}

/// Fills in the auction's result and starts the winner's 48h raise window.
#[instrument(skip(db))]
pub async fn open_rfa_raise_window<C>(
    rfa_resolution_id: i64,
    auction_result: ClosedRfaAuctionResult,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    let mut rfa_resolution_to_update: rfa_resolution::ActiveModel =
        find_rfa_resolution_by_id(rfa_resolution_id, db)
            .await?
            .into();
    rfa_resolution_to_update.auction_id = ActiveValue::Set(Some(auction_result.auction_id));
    rfa_resolution_to_update.winning_team_id =
        ActiveValue::Set(Some(auction_result.winning_team_id));
    rfa_resolution_to_update.final_bid = ActiveValue::Set(Some(auction_result.final_bid));
    rfa_resolution_to_update.final_bid_at = ActiveValue::Set(Some(auction_result.final_bid_at));
    rfa_resolution_to_update.raise_deadline_at =
        ActiveValue::Set(Some(auction_result.raise_deadline_at));
    rfa_resolution_to_update.status = ActiveValue::Set(RfaResolutionStatus::AwaitingRaise);
    Ok(rfa_resolution_to_update.update(db).await?)
}

/// Closes the raise window and starts the original owner's 48h window. `maybe_raised_bid` is NULL
/// when the winner stood pat.
#[instrument(skip(db))]
pub async fn open_rfa_match_window<C>(
    rfa_resolution_id: i64,
    maybe_raised_bid: Option<i16>,
    match_deadline_at: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    let mut rfa_resolution_to_update: rfa_resolution::ActiveModel =
        find_rfa_resolution_by_id(rfa_resolution_id, db)
            .await?
            .into();
    if let Some(raised_bid) = maybe_raised_bid {
        rfa_resolution_to_update.raised_bid = ActiveValue::Set(Some(raised_bid));
    }
    rfa_resolution_to_update.match_deadline_at = ActiveValue::Set(Some(match_deadline_at));
    rfa_resolution_to_update.status = ActiveValue::Set(RfaResolutionStatus::AwaitingMatch);
    Ok(rfa_resolution_to_update.update(db).await?)
}

/// Stamps the resolution's final state. `final_status` is one of `Resolved`, `Declined`,
/// `NoBidResigned` or `NoBidToAuction`.
#[instrument(skip(db))]
pub async fn finish_rfa_resolution<C>(
    rfa_resolution_id: i64,
    final_status: RfaResolutionStatus,
    resolved_at: DateTimeWithTimeZone,
    db: &C,
) -> Result<rfa_resolution::Model>
where
    C: ConnectionTrait,
{
    let mut rfa_resolution_to_update: rfa_resolution::ActiveModel =
        find_rfa_resolution_by_id(rfa_resolution_id, db)
            .await?
            .into();
    rfa_resolution_to_update.status = ActiveValue::Set(final_status);
    rfa_resolution_to_update.resolved_at = ActiveValue::Set(Some(resolved_at));
    Ok(rfa_resolution_to_update.update(db).await?)
}

/// The compensation a decline owes. `forfeited_draft_pick_id` is the winner's choice among the
/// eligible picks, and is NULL until they make it.
#[derive(Clone, Copy, Debug)]
pub struct NewRfaCompensationPick {
    pub rfa_resolution_id: i64,
    pub required_round: i16,
    pub forfeited_draft_pick_id: Option<i64>,
    /// The original owner, who receives the pick.
    pub to_team_id: i64,
    /// The winning bidder, who gives up the pick.
    pub from_team_id: i64,
}

#[instrument(skip(db))]
pub async fn insert_rfa_compensation_pick<C>(
    new_rfa_compensation_pick: NewRfaCompensationPick,
    db: &C,
) -> Result<rfa_compensation_pick::Model>
where
    C: ConnectionTrait,
{
    let rfa_compensation_pick_to_insert = rfa_compensation_pick::ActiveModel {
        id: ActiveValue::NotSet,
        rfa_resolution_id: ActiveValue::Set(new_rfa_compensation_pick.rfa_resolution_id),
        required_round: ActiveValue::Set(new_rfa_compensation_pick.required_round),
        forfeited_draft_pick_id: ActiveValue::Set(
            new_rfa_compensation_pick.forfeited_draft_pick_id,
        ),
        to_team_id: ActiveValue::Set(new_rfa_compensation_pick.to_team_id),
        from_team_id: ActiveValue::Set(new_rfa_compensation_pick.from_team_id),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };
    Ok(rfa_compensation_pick_to_insert.insert(db).await?)
}

/// The compensation row a resolution owes, if a decline has created one.
#[instrument(skip(db))]
pub async fn find_rfa_compensation_pick_for_resolution<C>(
    rfa_resolution_id: i64,
    db: &C,
) -> Result<Option<rfa_compensation_pick::Model>>
where
    C: ConnectionTrait,
{
    let maybe_rfa_compensation_pick = rfa_compensation_pick::Entity::find()
        .filter(rfa_compensation_pick::Column::RfaResolutionId.eq(rfa_resolution_id))
        .one(db)
        .await?;
    Ok(maybe_rfa_compensation_pick)
}
