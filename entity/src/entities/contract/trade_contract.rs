use sea_orm::ActiveValue;

use crate::contract;

pub fn trade_contract_to_team(
    contract_model: &contract::Model,
    new_team_id: i64,
) -> contract::ActiveModel {
    let mut new_contract: contract::ActiveModel = contract_model.clone().into();
    new_contract.id = ActiveValue::NotSet;
    new_contract.team_id = ActiveValue::Set(Some(new_team_id));
    new_contract.previous_contract_id = ActiveValue::Set(Some(contract_model.id));

    new_contract
}
