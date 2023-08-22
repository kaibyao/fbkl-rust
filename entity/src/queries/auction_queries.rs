use std::fmt::Debug;

use chrono::Days;
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use sea_orm::{
    prelude::DateTimeWithTimeZone, ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait,
    EntityTrait, QueryFilter,
};
use tracing::instrument;

use crate::{
    auction::{self, AuctionType},
    auction_bid,
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

/// Creates & inserts a new auction with given arguments.
#[instrument]
pub async fn insert_new_auction<C>(
    contract_id: i64,
    auction_type: AuctionType,
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
        .expect("There's no way we've reached the end of time itself, right?");
    let fixed_end_timestamp = fixed_end_datetime.unwrap_or_else(|| {
        start_datetime
            .checked_add_days(Days::new(2))
            .expect("There's no way we've reached the end of time itself, right?")
    });

    let auction_model_to_insert = auction::ActiveModel {
        id: ActiveValue::NotSet,
        auction_type: ActiveValue::Set(auction_type),
        minimum_bid_amount: ActiveValue::Set(minimum_bid_amount),
        start_timestamp: ActiveValue::Set(start_datetime),
        soft_end_timestamp: ActiveValue::Set(soft_end_timestamp),
        fixed_end_timestamp: ActiveValue::Set(fixed_end_timestamp),
        contract_id: ActiveValue::Set(contract_id),
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
