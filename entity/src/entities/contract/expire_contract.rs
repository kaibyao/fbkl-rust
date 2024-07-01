use crate::contract;
use color_eyre::{eyre::bail, Result};
use sea_orm::ActiveValue;

use super::{ContractStatus, ContractKind};

/// Creates a new contract from the given one, where the contract is expired.
pub fn expire_contract(current_contract: &contract::Model) -> Result<contract::ActiveModel> {
    if current_contract.status != ContractStatus::Active {
        bail!(
            "Cannot expire a replaced or expired contract. Contract:\n{:#?}",
            current_contract
        );
    }

    let new_contract = contract::ActiveModel {
        id: ActiveValue::NotSet,
        year_number: ActiveValue::Set(1),
        kind: ActiveValue::Set(ContractKind::FreeAgent),
        is_ir: ActiveValue::Set(false),
        salary: ActiveValue::Set(1),
        end_of_season_year: ActiveValue::Set(current_contract.end_of_season_year),
        status: ActiveValue::Set(ContractStatus::Expired),
        league_id: ActiveValue::Set(current_contract.league_id),
        league_player_id: ActiveValue::Set(current_contract.league_player_id),
        player_id: ActiveValue::Set(current_contract.player_id),
        previous_contract_id: ActiveValue::Set(Some(current_contract.id)),
        original_contract_id: ActiveValue::Set(current_contract.original_contract_id),
        team_id: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    Ok(new_contract)
}
