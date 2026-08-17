//! Reads/writes for the RFA raise/match handshake rows (rules §15.2, §15.3).
//!
//! Every rule about who may raise, name a pick, match or decline lives in `logic`; these are plain
//! row accessors. The scheduler drives both windows off
//! [`find_rfa_resolutions_with_expired_window`].

use std::{collections::HashSet, fmt::Debug};

use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectionTrait, EntityTrait, JoinType,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, prelude::DateTimeWithTimeZone,
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

/// Resolutions whose open window has run out by `now` — the scheduler's work list for both
/// handshake timeouts. The row's own status says which window it is.
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

/// The compensation a decline would owe, written by the bid that named the pick (rules §15.3.3).
#[derive(Clone, Copy, Debug)]
pub struct NewRfaCompensationPick {
    pub rfa_resolution_id: i64,
    pub required_round: i16,
    pub forfeited_draft_pick_id: i64,
    /// The original owner, who receives the pick.
    pub to_team_id: i64,
    /// The team currently leading the bid, which gives up the pick.
    pub from_team_id: i64,
}

/// Writes what the resolution's current leader would forfeit, replacing whatever the last bid said.
///
/// A resolution owes at most one pick, so the row is overwritten rather than added to: being
/// outbid is what frees the previous leader's pick for his other bids.
#[instrument(skip(db))]
pub async fn upsert_rfa_compensation_pick<C>(
    new_rfa_compensation_pick: NewRfaCompensationPick,
    db: &C,
) -> Result<rfa_compensation_pick::Model>
where
    C: ConnectionTrait,
{
    let maybe_existing_row =
        find_rfa_compensation_pick_for_resolution(new_rfa_compensation_pick.rfa_resolution_id, db)
            .await?;
    let mut rfa_compensation_pick_to_save = rfa_compensation_pick::ActiveModel {
        rfa_resolution_id: ActiveValue::Set(new_rfa_compensation_pick.rfa_resolution_id),
        required_round: ActiveValue::Set(new_rfa_compensation_pick.required_round),
        forfeited_draft_pick_id: ActiveValue::Set(
            new_rfa_compensation_pick.forfeited_draft_pick_id,
        ),
        to_team_id: ActiveValue::Set(new_rfa_compensation_pick.to_team_id),
        from_team_id: ActiveValue::Set(new_rfa_compensation_pick.from_team_id),
        ..Default::default()
    };
    match maybe_existing_row {
        Some(existing_row) => {
            rfa_compensation_pick_to_save.id = ActiveValue::Unchanged(existing_row.id);
            Ok(rfa_compensation_pick_to_save.update(db).await?)
        }
        None => Ok(rfa_compensation_pick_to_save.insert(db).await?),
    }
}

/// The compensation row a resolution owes, once a bid has named a pick for it.
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

/// The picks `team_id` has already named on the season's other restricted free agents, and so
/// cannot name again (rules §15.3.3).
///
/// A debt is outstanding from the bid that names it until the handshake ends: a decline has moved
/// the pick by then, and a match cancels the debt and leaves its row behind as a record. Being
/// outbid frees the pick without any row of its own, because the new leader's bid overwrites it.
#[instrument(skip(db))]
pub async fn find_reserved_compensation_pick_ids<C>(
    league_id: i64,
    end_of_season_year: i16,
    team_id: i64,
    excluded_rfa_resolution_id: i64,
    db: &C,
) -> Result<HashSet<i64>>
where
    C: ConnectionTrait,
{
    let reserved_picks: Vec<i64> = rfa_compensation_pick::Entity::find()
        .join(
            JoinType::InnerJoin,
            rfa_compensation_pick::Relation::RfaResolution.def(),
        )
        .filter(rfa_resolution::Column::LeagueId.eq(league_id))
        .filter(rfa_resolution::Column::EndOfSeasonYear.eq(end_of_season_year))
        .filter(rfa_resolution::Column::Status.is_in([
            RfaResolutionStatus::AwaitingAuction,
            RfaResolutionStatus::AwaitingRaise,
            RfaResolutionStatus::AwaitingMatch,
        ]))
        .filter(rfa_compensation_pick::Column::FromTeamId.eq(team_id))
        .filter(rfa_compensation_pick::Column::RfaResolutionId.ne(excluded_rfa_resolution_id))
        .select_only()
        .column(rfa_compensation_pick::Column::ForfeitedDraftPickId)
        .into_tuple()
        .all(db)
        .await?;
    Ok(reserved_picks.into_iter().collect())
}
