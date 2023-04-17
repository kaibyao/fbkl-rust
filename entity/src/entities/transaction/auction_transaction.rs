use crate::{deadline, transaction};
use sea_orm::ActiveValue;

use super::TransactionType;

pub fn new_auction_transaction(
    deadline_model: &deadline::Model,
    auction_id: i64,
) -> transaction::ActiveModel {
    transaction::ActiveModel {
        end_of_season_year: ActiveValue::Set(deadline_model.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::AuctionDone),
        league_id: ActiveValue::Set(deadline_model.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        auction_id: ActiveValue::Set(Some(auction_id)),
        ..Default::default()
    }
}
