use color_eyre::{eyre::bail, Result};
use sea_orm::ActiveValue;

use crate::contract::{self, ContractStatus, ContractType};

static APPLICABLE_CONTRACT_TYPES: [ContractType; 2] = [
    ContractType::RookieDevelopment,
    ContractType::RookieDevelopmentInternational,
];

/// Creates a new contract from the given Rookie Development (optional: International) contract where the contract is now converted to a standard Rookie contract.
pub fn create_rookie_contract_from_rd(
    current_contract: &contract::Model,
) -> Result<contract::ActiveModel> {
    if !APPLICABLE_CONTRACT_TYPES.contains(&current_contract.contract_type) {
        bail!(
            "Can only create a rookie contract from a Rookie Development (International) contract."
        );
    }
    if current_contract.status != ContractStatus::Active {
        bail!(
            "Cannot advance a replaced or expired contract. Contract:\n{:#?}",
            current_contract
        );
    }

    let new_contract = contract::ActiveModel {
        id: ActiveValue::NotSet,
        contract_year: ActiveValue::Set(1),
        contract_type: ActiveValue::Set(contract::ContractType::Rookie),
        is_ir: ActiveValue::Set(current_contract.is_ir),
        salary: ActiveValue::Set(current_contract.salary),
        season_end_year: ActiveValue::Set(current_contract.season_end_year),
        status: ActiveValue::Set(current_contract.status),
        league_id: ActiveValue::Set(current_contract.league_id),
        league_player_id: ActiveValue::Set(current_contract.league_player_id),
        player_id: ActiveValue::Set(current_contract.player_id),
        previous_contract_id: ActiveValue::Set(Some(current_contract.id)),
        original_contract_id: ActiveValue::Set(current_contract.original_contract_id),
        team_id: ActiveValue::Set(current_contract.team_id),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    Ok(new_contract)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use color_eyre::Result;
    use once_cell::sync::Lazy;
    use sea_orm::ActiveValue;

    use crate::contract::{
        self, rookie_activation::create_rookie_contract_from_rd, ContractStatus, ContractType,
    };

    static NOW: Lazy<DateTime<FixedOffset>> = Lazy::new(|| {
        DateTime::parse_from_str("2023 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
            .unwrap()
    });

    fn generate_contract() -> contract::Model {
        contract::Model {
            id: 1,
            contract_type: ContractType::RookieDevelopment,
            contract_year: 1,
            salary: 4,
            is_ir: false,
            season_end_year: 2023,
            league_id: 1,
            league_player_id: None,
            player_id: Some(1),
            original_contract_id: Some(1),
            previous_contract_id: None,
            team_id: Some(1),
            status: ContractStatus::Active,
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
        }
    }

    #[test]
    fn rd2_activate() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_year = 2;

        let advanced_contract = create_rookie_contract_from_rd(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Rookie)
        );
        assert_eq!(
            advanced_contract.salary,
            ActiveValue::Set(test_contract.salary)
        );

        Ok(())
    }

    #[test]
    fn rd3_activate_in_season() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_year = 3;

        let advanced_contract = create_rookie_contract_from_rd(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Rookie)
        );
        assert_eq!(
            advanced_contract.salary,
            ActiveValue::Set(test_contract.salary)
        );

        Ok(())
    }
}
