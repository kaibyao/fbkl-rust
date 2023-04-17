use crate::{deadline, transaction};
use sea_orm::ActiveValue;

use super::TransactionType;

pub fn new_keeper_deadline_transaction(
    keeper_deadline_model: &deadline::Model,
) -> transaction::ActiveModel {
    transaction::ActiveModel {
        end_of_season_year: ActiveValue::Set(keeper_deadline_model.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::PreseasonKeeper),
        league_id: ActiveValue::Set(keeper_deadline_model.league_id),
        deadline_id: ActiveValue::Set(keeper_deadline_model.id),
        ..Default::default()
    }
}
