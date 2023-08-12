use crate::{deadline, trade, transaction};
use sea_orm::ActiveValue;

use super::TransactionType;

pub fn new_trade_transaction(
    deadline_model: &deadline::Model,
    trade_model: &trade::Model,
) -> transaction::ActiveModel {
    transaction::ActiveModel {
        end_of_season_year: ActiveValue::Set(trade_model.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::Trade),
        league_id: ActiveValue::Set(trade_model.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        trade_id: ActiveValue::Set(Some(trade_model.id)),
        ..Default::default()
    }
}
