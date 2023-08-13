use crate::{deadline, transaction};
use sea_orm::ActiveValue;

use super::TransactionType;

pub fn new_trade_transaction(
    deadline_model: &deadline::Model,
    trade_id: i64,
) -> transaction::ActiveModel {
    transaction::ActiveModel {
        end_of_season_year: ActiveValue::Set(deadline_model.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::Trade),
        league_id: ActiveValue::Set(deadline_model.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        trade_id: ActiveValue::Set(Some(trade_id)),
        ..Default::default()
    }
}
