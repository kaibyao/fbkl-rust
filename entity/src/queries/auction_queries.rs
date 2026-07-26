use std::fmt::Debug;

use chrono::Days;
use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, JoinType,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, prelude::DateTimeWithTimeZone,
};
use tracing::instrument;

use crate::{
    auction::{self, AuctionKind},
    auction_bid, contract,
    queries::pagination::{Paged, fetch_page},
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
        .order_by_asc(auction::Column::FixedEndTimestamp)
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

/// Creates & inserts a new auction with given arguments.
#[instrument]
pub async fn insert_new_auction<C>(
    contract_id: i64,
    kind: AuctionKind,
    minimum_bid_amount: i16,
    start_datetime: DateTimeWithTimeZone,
    fixed_end_datetime: Option<DateTimeWithTimeZone>,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    let soft_end_timestamp = start_datetime
        .checked_add_days(Days::new(1))
        .ok_or_else(|| eyre!("auction start_datetime + 1 day overflowed: {start_datetime}"))?;
    let fixed_end_timestamp = match fixed_end_datetime {
        Some(fixed_end) => fixed_end,
        None => start_datetime
            .checked_add_days(Days::new(2))
            .ok_or_else(|| eyre!("auction start_datetime + 2 days overflowed: {start_datetime}"))?,
    };

    let auction_model_to_insert = auction::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(kind),
        minimum_bid_amount: ActiveValue::Set(minimum_bid_amount),
        start_timestamp: ActiveValue::Set(start_datetime),
        soft_end_timestamp: ActiveValue::Set(soft_end_timestamp),
        fixed_end_timestamp: ActiveValue::Set(fixed_end_timestamp),
        contract_id: ActiveValue::Set(contract_id),
        transaction_id: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };
    let inserted_model = auction_model_to_insert.insert(db).await?;

    Ok(inserted_model)
}

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
    let auction_model = find_auction_by_id(auction_id, db).await?;
    let maybe_latest_bid = auction_model.get_latest_bid(db).await?;

    // validate bid amount
    match maybe_latest_bid {
        None => {
            if bid_amount < auction_model.minimum_bid_amount {
                bail!(
                    "Auction bid amount ({}) must be greater than the starting price ({}).",
                    bid_amount,
                    auction_model.minimum_bid_amount
                );
            }
        }
        Some(latest_auction_bid) => {
            if bid_amount <= latest_auction_bid.bid_amount {
                bail!(
                    "Auction bid amount ({}) must be greater than the previous bid ({}).",
                    bid_amount,
                    latest_auction_bid.bid_amount
                );
            }
        }
    }

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
