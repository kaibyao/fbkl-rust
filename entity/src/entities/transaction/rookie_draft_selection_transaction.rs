use crate::{deadline, transaction};
use sea_orm::ActiveValue;

use super::TransactionType;

pub fn new_rookie_draft_selection_transaction(
    deadline_model: &deadline::Model,
    rookie_draft_selection_id: i64,
) -> transaction::ActiveModel {
    transaction::ActiveModel {
        end_of_season_year: ActiveValue::Set(deadline_model.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::RookieDraftSelection),
        league_id: ActiveValue::Set(deadline_model.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        rookie_draft_selection_id: ActiveValue::Set(Some(rookie_draft_selection_id)),
        ..Default::default()
    }
}
