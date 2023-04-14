use std::fmt::Debug;

use chrono::Days;
use color_eyre::Result;
use sea_orm::{
    prelude::DateTimeWithTimeZone, ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionTrait,
};
use tracing::instrument;

use crate::auction::{self, AuctionType};

/// Creates & inserts a new auction with given arguments.
#[instrument]
pub async fn insert_new_auction<C>(
    contract_id: i64,
    auction_type: AuctionType,
    start_datetime: DateTimeWithTimeZone,
    fixed_end_datetime: Option<DateTimeWithTimeZone>,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
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
