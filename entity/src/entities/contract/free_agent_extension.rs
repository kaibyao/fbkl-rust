//! Handles contract entity generation for signing RFA and UFA contracts to a team.

use crate::contract::{self, ContractType};
use color_eyre::{eyre::bail, Result};
use sea_orm::ActiveValue;

use super::ContractStatus;

static APPLICABLE_CONTRACT_TYPES: [ContractType; 3] = [
    ContractType::RestrictedFreeAgent,
    ContractType::UnrestrictedFreeAgentOriginalTeam,
    ContractType::UnrestrictedFreeAgentVeteran,
];

/// Creates a new Veteran or Rookie Extension contract from the given RFA or UFA contract as a result of a team winning the contract during the Preseason Veteran Auction.
pub fn sign_rfa_or_ufa_contract_to_team(
    fa_contract: &contract::Model,
    signing_team_id: i64,
    winning_bid_amount: i16,
) -> Result<contract::ActiveModel> {
    if !APPLICABLE_CONTRACT_TYPES.contains(&fa_contract.contract_type) {
        bail!(
            "Can only sign an RFA or UFA contract (given contract type: {}).",
            fa_contract.contract_type
        );
    }
    if fa_contract.status != ContractStatus::Active {
        bail!(
            "Cannot sign an extension for a replaced or expired contract. Contract:\n{:#?}",
            fa_contract
        );
    }

    // Defaults for signing to a new team
    let mut new_contract_year = 1;
    let mut new_contract_type = ContractType::Veteran;
    let mut new_salary = winning_bid_amount;

    // Overwrite defaults if signing to original owning team
    if fa_contract.team_id == Some(signing_team_id) {
        match fa_contract.contract_type {
            ContractType::RestrictedFreeAgent => {
                new_contract_year = 4;
                new_contract_type = ContractType::RookieExtension;
                new_salary = get_salary_discounted_by_10_percent(winning_bid_amount);
            }
            ContractType::UnrestrictedFreeAgentOriginalTeam => {
                new_salary = get_salary_discounted_by_20_percent(winning_bid_amount);
            }
            ContractType::UnrestrictedFreeAgentVeteran => {
                new_salary = get_salary_discounted_by_10_percent(winning_bid_amount);
            }
            _ => bail!("Validation already handled"),
        }
    }

    let new_contract = contract::ActiveModel {
        id: ActiveValue::NotSet,
        contract_year: ActiveValue::Set(new_contract_year),
        contract_type: ActiveValue::Set(new_contract_type),
        is_ir: ActiveValue::Set(fa_contract.is_ir),
        salary: ActiveValue::Set(new_salary),
        season_end_year: ActiveValue::Set(fa_contract.season_end_year),
        status: ActiveValue::Set(fa_contract.status),
        league_id: ActiveValue::Set(fa_contract.league_id),
        league_player_id: ActiveValue::Set(fa_contract.league_player_id),
        player_id: ActiveValue::Set(fa_contract.player_id),
        previous_contract_id: ActiveValue::Set(Some(fa_contract.id)),
        original_contract_id: ActiveValue::Set(fa_contract.original_contract_id),
        team_id: ActiveValue::Set(fa_contract.team_id),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    Ok(new_contract)
}

fn get_salary_discounted_by_10_percent(salary: i16) -> i16 {
    let discount_amount_rounded_up = (f32::from(salary) * 0.1).ceil();
    salary - (discount_amount_rounded_up as i16)
}

fn get_salary_discounted_by_20_percent(salary: i16) -> i16 {
    let discount_amount_rounded_up = (f32::from(salary) * 0.2).ceil();
    salary - (discount_amount_rounded_up as i16)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use color_eyre::Result;
    use once_cell::sync::Lazy;
    use sea_orm::ActiveValue;

    use crate::contract::{
        free_agent_extension::sign_rfa_or_ufa_contract_to_team, ContractStatus, ContractType, Model,
    };

    static NOW: Lazy<DateTime<FixedOffset>> = Lazy::new(|| {
        DateTime::parse_from_str("2023 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
            .unwrap()
    });

    fn generate_contract() -> Model {
        Model {
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
    fn resign_rfa_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::RestrictedFreeAgent;
        test_contract.salary = 4;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 11)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(4));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::RookieExtension)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(9));

        Ok(())
    }

    #[test]
    fn sign_rfa_different_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::RestrictedFreeAgent;
        test_contract.salary = 4;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 11)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(11));

        Ok(())
    }

    #[test]
    fn sign_ufa20_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::UnrestrictedFreeAgentOriginalTeam;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 33)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(26));

        Ok(())
    }

    #[test]
    fn sign_ufa20_different_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::UnrestrictedFreeAgentOriginalTeam;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 33)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(33));

        Ok(())
    }

    #[test]
    fn sign_ufa10_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::UnrestrictedFreeAgentVeteran;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 33)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(29));

        Ok(())
    }

    #[test]
    fn sign_ufa10_new_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::UnrestrictedFreeAgentVeteran;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 33)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(33));

        Ok(())
    }
}
