use crate::contract;
use color_eyre::{eyre::bail, Result};
use sea_orm::ActiveValue;

use super::{ContractStatus, ContractKind};

static APPLICABLE_CONTRACT_TYPES: [ContractKind; 2] =
    [ContractKind::Veteran, ContractKind::FreeAgent];

/// Creates a new contract from the given one, where the contract is signed to a team.
pub fn sign_veteran_contract(
    current_contract: &contract::Model,
    team_id: i64,
    salary: i16,
) -> Result<contract::ActiveModel> {
    if !APPLICABLE_CONTRACT_TYPES.contains(&current_contract.kind) {
        bail!(
            "Can only sign a veteran or free agent contract (given contract type: {:?}).",
            current_contract.kind
        );
    }
    if current_contract.status != ContractStatus::Active {
        bail!(
            "Cannot sign a veteran contract that's replaced or expired. Contract:\n{:#?}",
            current_contract
        );
    }

    let new_contract = contract::ActiveModel {
        id: ActiveValue::NotSet,
        year_number: ActiveValue::Set(1),
        kind: ActiveValue::Set(ContractKind::Veteran),
        is_ir: ActiveValue::Set(false),
        salary: ActiveValue::Set(salary),
        end_of_season_year: ActiveValue::Set(current_contract.end_of_season_year),
        status: ActiveValue::Set(ContractStatus::Active),
        league_id: ActiveValue::Set(current_contract.league_id),
        league_player_id: ActiveValue::Set(current_contract.league_player_id),
        player_id: ActiveValue::Set(current_contract.player_id),
        previous_contract_id: ActiveValue::Set(Some(current_contract.id)),
        original_contract_id: ActiveValue::Set(current_contract.original_contract_id),
        team_id: ActiveValue::Set(Some(team_id)),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    Ok(new_contract)
}
