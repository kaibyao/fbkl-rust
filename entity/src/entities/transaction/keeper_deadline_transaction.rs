use crate::{deadline, transaction};
use color_eyre::Result;
use sea_orm::ActiveValue;

use super::TransactionType;

pub fn new_keeper_deadline_transaction(
    keeper_deadline_model: &deadline::Model,
) -> Result<transaction::ActiveModel> {
    let transaction_model = transaction::ActiveModel {
        end_of_season_year: ActiveValue::Set(keeper_deadline_model.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::PreseasonKeeper),
        league_id: ActiveValue::Set(keeper_deadline_model.league_id),
        deadline_id: ActiveValue::Set(keeper_deadline_model.id),
        ..Default::default()
    };
    Ok(transaction_model)
}
