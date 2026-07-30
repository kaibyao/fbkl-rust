use std::fmt::Debug;

use chrono::{Days, TimeDelta};
use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectionTrait, EntityTrait, JoinType,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, prelude::DateTimeWithTimeZone,
};
use tracing::instrument;

use crate::{
    auction::{self, AuctionKind, AuctionStatus},
    auction_bid, contract,
    queries::pagination::{Paged, fetch_page},
    team_user,
};

#[instrument]
pub async fn find_auction_by_id<C>(auction_id: i64, db: &C) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    let maybe_auction_model = auction::Entity::find()
        .filter(auction::Column::Id.eq(auction_id))
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find auction with id: {}", auction_id))?;
    Ok(maybe_auction_model)
}

/// Same as [`find_auction_by_id`] but takes a row lock, so racing bids on one auction serialize.
/// Only meaningful inside a db transaction.
#[instrument]
pub async fn find_auction_by_id_for_update<C>(auction_id: i64, db: &C) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    auction::Entity::find_by_id(auction_id)
        .lock_exclusive()
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find auction with id: {}", auction_id))
}

/// The team's currently-winning bids (`(auction_id, bid_amount)`) across the league/season's `Open`
/// auctions — the commitments rules §6.4.1 counts against a new bid.
#[instrument]
pub async fn find_winning_bids_for_team<C>(
    team_id: i64,
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<(i64, i16)>>
where
    C: ConnectionTrait + Debug,
{
    let bids: Vec<(i64, i16, i64)> = auction_bid::Entity::find()
        .join(JoinType::InnerJoin, auction_bid::Relation::Auction.def())
        .join(JoinType::InnerJoin, auction::Relation::Contract.def())
        .join(JoinType::InnerJoin, auction_bid::Relation::TeamUser.def())
        .filter(auction::Column::Status.eq(AuctionStatus::Open))
        .filter(contract::Column::LeagueId.eq(league_id))
        .filter(contract::Column::EndOfSeasonYear.eq(end_of_season_year))
        .select_only()
        .column(auction_bid::Column::AuctionId)
        .column(auction_bid::Column::BidAmount)
        .column(team_user::Column::TeamId)
        .order_by_asc(auction_bid::Column::AuctionId)
        .order_by_desc(auction_bid::Column::CreatedAt)
        .order_by_desc(auction_bid::Column::Id)
        .into_tuple()
        .all(db)
        .await?;

    // rows are grouped per auction with the latest bid first, so the first row per auction wins it
    let mut winning_bids = Vec::new();
    let mut previous_auction_id = None;
    for (auction_id, bid_amount, bidding_team_id) in bids {
        if previous_auction_id == Some(auction_id) {
            continue;
        }
        previous_auction_id = Some(auction_id);
        if bidding_team_id == team_id {
            winning_bids.push((auction_id, bid_amount));
        }
    }
    Ok(winning_bids)
}

/// Auctions in the league/season that have not settled yet — `transaction_id` is NULL until a
/// winning bid is signed. The league scope comes from the auctioned contract.
#[instrument]
pub async fn find_open_auctions_in_league<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<auction::Model>>
where
    C: ConnectionTrait + Debug,
{
    let auction_models = auction::Entity::find()
        .join(JoinType::InnerJoin, auction::Relation::Contract.def())
        .filter(auction::Column::TransactionId.is_null())
        .filter(contract::Column::LeagueId.eq(league_id))
        .filter(contract::Column::EndOfSeasonYear.eq(end_of_season_year))
        .order_by_asc(auction::Column::CloseAtTimestamp)
        .all(db)
        .await?;
    Ok(auction_models)
}

/// Whether an open auction's bidding is over: the close time has passed with no bid in the last
/// hour (rules §6.4.4 / §8.3.1 / §8.3.2), or its all-bid deadline has passed.
///
/// The last-hour reprieve is why a lapsed close time alone is not enough: bids landing within an
/// hour of each other keep an auction alive past it. Only in-season FA auctions have an all-bid
/// deadline; it closes the auction on its own.
pub fn is_due_for_close(
    now: DateTimeWithTimeZone,
    close_at: DateTimeWithTimeZone,
    maybe_all_bid_deadline: Option<DateTimeWithTimeZone>,
    maybe_last_bid_at: Option<DateTimeWithTimeZone>,
) -> bool {
    if maybe_all_bid_deadline.is_some_and(|all_bid_deadline| all_bid_deadline <= now) {
        return true;
    }
    let quiet_for_an_hour =
        maybe_last_bid_at.is_none_or(|last_bid_at| now - last_bid_at >= TimeDelta::hours(1));
    close_at <= now && quiet_for_an_hour
}

/// `Open` auctions whose bidding is over per [`is_due_for_close`], with the contract they auction.
#[instrument]
pub async fn find_auctions_due_for_close<C>(
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<Vec<(auction::Model, contract::Model)>>
where
    C: ConnectionTrait + Debug,
{
    // Either timestamp can trigger a close, so both are candidates to narrow in the database.
    let rows = auction::Entity::find()
        .find_also_related(contract::Entity)
        .filter(auction::Column::Status.eq(AuctionStatus::Open))
        .filter(
            Condition::any()
                .add(auction::Column::CloseAtTimestamp.lte(now))
                .add(auction::Column::AllBidDeadlineTimestamp.lte(now)),
        )
        .order_by_asc(auction::Column::CloseAtTimestamp)
        .all(db)
        .await?;

    let mut due_auctions = Vec::new();
    for (auction_model, maybe_contract) in rows {
        let Some(contract_model) = maybe_contract else {
            continue;
        };
        let maybe_last_bid_at = auction_model
            .get_latest_bid(db)
            .await?
            .map(|bid| bid.created_at);
        if is_due_for_close(
            now,
            auction_model.close_at_timestamp,
            auction_model.all_bid_deadline_timestamp,
            maybe_last_bid_at,
        ) {
            due_auctions.push((auction_model, contract_model));
        }
    }
    Ok(due_auctions)
}

/// The distinct `(league_id, end_of_season_year)` pairs that currently have an `Open` auction of
/// the given kind — the leagues a periodic auction tick has work for.
#[instrument]
pub async fn find_league_seasons_with_open_auctions<C>(
    kind: AuctionKind,
    db: &C,
) -> Result<Vec<(i64, i16)>>
where
    C: ConnectionTrait + Debug,
{
    let league_seasons = auction::Entity::find()
        .join(JoinType::InnerJoin, auction::Relation::Contract.def())
        .filter(auction::Column::Status.eq(AuctionStatus::Open))
        .filter(auction::Column::Kind.eq(kind))
        .select_only()
        .column(contract::Column::LeagueId)
        .column(contract::Column::EndOfSeasonYear)
        .distinct()
        .into_tuple()
        .all(db)
        .await?;
    Ok(league_seasons)
}

/// The auction for a given pooled contract, if one was already opened.
#[instrument]
pub async fn find_auction_by_contract_id<C>(
    contract_id: i64,
    db: &C,
) -> Result<Option<auction::Model>>
where
    C: ConnectionTrait + Debug,
{
    let auction_model = auction::Entity::find()
        .filter(auction::Column::ContractId.eq(contract_id))
        .one(db)
        .await?;
    Ok(auction_model)
}

/// `Open` auctions of the given kind that have no bids yet and were last touched before
/// `unchanged_before`.
///
/// The timestamp bound keeps a freshly-opened auction from sliding a tier on its first day, and
/// keeps an auction already slid today from sliding again on the next tick (rules §6.3.4).
#[instrument]
pub async fn find_unbid_open_auctions<C>(
    league_id: i64,
    end_of_season_year: i16,
    kind: AuctionKind,
    unchanged_before: DateTimeWithTimeZone,
    db: &C,
) -> Result<Vec<auction::Model>>
where
    C: ConnectionTrait + Debug,
{
    let auction_models = auction::Entity::find()
        .join(JoinType::InnerJoin, auction::Relation::Contract.def())
        .join(JoinType::LeftJoin, auction::Relation::AuctionBid.def())
        .filter(auction::Column::Status.eq(AuctionStatus::Open))
        .filter(auction::Column::Kind.eq(kind))
        .filter(auction::Column::StartTimestamp.lte(unchanged_before))
        .filter(auction::Column::UpdatedAt.lte(unchanged_before))
        .filter(auction_bid::Column::Id.is_null())
        .filter(contract::Column::LeagueId.eq(league_id))
        .filter(contract::Column::EndOfSeasonYear.eq(end_of_season_year))
        .all(db)
        .await?;
    Ok(auction_models)
}

/// One page of an auction's bid history, newest bid first.
#[instrument]
pub async fn find_auction_bids<C>(
    auction_id: i64,
    page: u64,
    page_size: u64,
    db: &C,
) -> Result<Paged<auction_bid::Model>>
where
    C: ConnectionTrait + Debug,
{
    let query = auction_bid::Entity::find()
        .filter(auction_bid::Column::AuctionId.eq(auction_id))
        .order_by_desc(auction_bid::Column::CreatedAt);

    fetch_page(query, page, page_size, db).await
}

/// Rewrites when an auction stops taking bids (rules §6.4.4 / §8.3.1).
#[instrument]
pub async fn set_auction_close_at<C>(
    auction_id: i64,
    new_close_at: DateTimeWithTimeZone,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut auction_to_update: auction::ActiveModel =
        find_auction_by_id(auction_id, db).await?.into();
    auction_to_update.close_at_timestamp = ActiveValue::Set(new_close_at);
    Ok(auction_to_update.update(db).await?)
}

/// Lowers an unbid veteran auction's minimum bid to the next tier (rules §6.3.4).
#[instrument]
pub async fn update_auction_minimum_bid<C>(
    auction_id: i64,
    new_minimum_bid_amount: i16,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut auction_to_update: auction::ActiveModel =
        find_auction_by_id(auction_id, db).await?.into();
    auction_to_update.minimum_bid_amount = ActiveValue::Set(new_minimum_bid_amount);
    Ok(auction_to_update.update(db).await?)
}

#[instrument]
pub async fn update_auction_status<C>(
    auction_id: i64,
    new_status: AuctionStatus,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut auction_to_update: auction::ActiveModel =
        find_auction_by_id(auction_id, db).await?.into();
    auction_to_update.status = ActiveValue::Set(new_status);
    Ok(auction_to_update.update(db).await?)
}

/// Creates & inserts a new auction with given arguments.
#[instrument]
pub async fn insert_new_auction<C>(
    contract_id: i64,
    kind: AuctionKind,
    minimum_bid_amount: i16,
    start_datetime: DateTimeWithTimeZone,
    maybe_all_bid_deadline: Option<DateTimeWithTimeZone>,
    maybe_original_owner_team_id: Option<i64>,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    let close_at_timestamp = start_datetime
        .checked_add_days(Days::new(1))
        .ok_or_else(|| eyre!("auction start_datetime + 1 day overflowed: {start_datetime}"))?;

    let auction_model_to_insert = auction::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(kind),
        status: ActiveValue::Set(AuctionStatus::Open),
        minimum_bid_amount: ActiveValue::Set(minimum_bid_amount),
        start_timestamp: ActiveValue::Set(start_datetime),
        close_at_timestamp: ActiveValue::Set(close_at_timestamp),
        all_bid_deadline_timestamp: ActiveValue::Set(maybe_all_bid_deadline),
        contract_id: ActiveValue::Set(contract_id),
        original_owner_team_id: ActiveValue::Set(maybe_original_owner_team_id),
        transaction_id: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };
    let inserted_model = auction_model_to_insert.insert(db).await?;

    Ok(inserted_model)
}

/// Pure insert — `logic::auction::place_auction_bid` owns every bid rule (rules §6.4, §8.3).
#[instrument]
pub async fn insert_auction_bid<C>(
    auction_id: i64,
    team_user_id: i64,
    bid_amount: i16,
    maybe_comment: Option<String>,
    db: &C,
) -> Result<auction_bid::Model>
where
    C: ConnectionTrait + Debug,
{
    let auction_bid_to_insert = auction_bid::ActiveModel {
        id: ActiveValue::NotSet,
        bid_amount: ActiveValue::Set(bid_amount),
        comment: ActiveValue::Set(maybe_comment),
        auction_id: ActiveValue::Set(auction_id),
        team_user_id: ActiveValue::Set(team_user_id),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };
    let inserted_auction_bid = auction_bid_to_insert.insert(db).await?;
    Ok(inserted_auction_bid)
}

#[cfg(test)]
mod tests {
    use super::{DateTimeWithTimeZone, is_due_for_close};

    fn at(time: &str) -> DateTimeWithTimeZone {
        format!("2024-11-17T{time}:00-06:00").parse().unwrap()
    }

    #[test]
    fn a_quiet_auction_closes_once_its_close_time_passes() {
        assert!(is_due_for_close(
            at("20:00"),
            at("19:30"),
            Some(at("23:00")),
            Some(at("18:00"))
        ));
    }

    #[test]
    fn an_auction_still_taking_bids_outlives_its_close_time() {
        assert!(!is_due_for_close(
            at("20:00"),
            at("19:30"),
            Some(at("23:00")),
            Some(at("19:40"))
        ));
    }

    #[test]
    fn an_hour_of_quiet_after_a_late_bid_closes_it() {
        assert!(is_due_for_close(
            at("20:40"),
            at("19:30"),
            Some(at("23:00")),
            Some(at("19:40"))
        ));
    }

    #[test]
    fn the_all_bid_deadline_closes_an_auction_even_mid_flurry() {
        // A bid a minute ago rolled the close time past the deadline; without this it never ends.
        assert!(is_due_for_close(
            at("23:00"),
            at("23:59"),
            Some(at("23:00")),
            Some(at("22:59"))
        ));
    }

    #[test]
    fn an_auction_without_an_all_bid_deadline_closes_on_its_close_time() {
        assert!(is_due_for_close(at("20:00"), at("19:30"), None, None));
        assert!(!is_due_for_close(at("19:00"), at("19:30"), None, None));
    }
}
