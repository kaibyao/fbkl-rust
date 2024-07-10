//! Handles contract entity generation for signing RFA and UFA contracts to a team.

use std::cmp;

use crate::contract::{self, ContractKind};
use color_eyre::{eyre::bail, Result};
use sea_orm::ActiveValue;

use super::ContractStatus;

static APPLICABLE_CONTRACT_TYPES: [ContractKind; 3] = [
    ContractKind::RestrictedFreeAgent,
    ContractKind::UnrestrictedFreeAgentOriginalTeam,
    ContractKind::UnrestrictedFreeAgentVeteran,
];

/// Creates a new Veteran or Rookie Extension contract from the given RFA or UFA contract as a result of a team winning the contract during the Preseason Veteran Auction.
pub fn sign_rfa_or_ufa_contract_to_team(
    fa_contract: &contract::Model,
    signing_team_id: i64,
    winning_bid_amount: i16,
) -> Result<contract::ActiveModel> {
    if !APPLICABLE_CONTRACT_TYPES.contains(&fa_contract.kind) {
        bail!(
            "Can only sign an RFA or UFA contract (given contract type: {:?}).",
            fa_contract.kind
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
    let mut new_contract_type = ContractKind::Veteran;
    let mut new_salary = winning_bid_amount;

    // Overwrite defaults if signing to original owning team
    if fa_contract.team_id == Some(signing_team_id) {
        match fa_contract.kind {
            ContractKind::RestrictedFreeAgent => {
                new_contract_year = 4;
                new_contract_type = ContractKind::RookieExtension;
                new_salary = get_salary_discounted_by_10_percent(winning_bid_amount);
            }
            ContractKind::UnrestrictedFreeAgentOriginalTeam => {
                new_salary = get_salary_discounted_by_20_percent(winning_bid_amount);
            }
            ContractKind::UnrestrictedFreeAgentVeteran => {
                new_salary = get_salary_discounted_by_10_percent(winning_bid_amount);
            }
            _ => bail!("Validation already handled"),
        }
    }

    let new_contract = contract::ActiveModel {
        id: ActiveValue::NotSet,
        year_number: ActiveValue::Set(new_contract_year),
        kind: ActiveValue::Set(new_contract_type),
        is_ir: ActiveValue::Set(fa_contract.is_ir),
        salary: ActiveValue::Set(new_salary),
        end_of_season_year: ActiveValue::Set(fa_contract.end_of_season_year),
        status: ActiveValue::Set(fa_contract.status),
        league_id: ActiveValue::Set(fa_contract.league_id),
        league_player_id: ActiveValue::Set(fa_contract.league_player_id),
        player_id: ActiveValue::Set(fa_contract.player_id),
        previous_contract_id: ActiveValue::Set(Some(fa_contract.id)),
        original_contract_id: ActiveValue::Set(fa_contract.original_contract_id),
        team_id: ActiveValue::Set(Some(signing_team_id)),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    Ok(new_contract)
}

fn get_salary_discounted_by_10_percent(salary: i16) -> i16 {
    let discount_amount_rounded_up = (f32::from(salary) * 0.1).ceil();
    let discounted_salary = salary - (discount_amount_rounded_up as i16);
    cmp::max(discounted_salary, 1)
}

fn get_salary_discounted_by_20_percent(salary: i16) -> i16 {
    let discount_amount_rounded_up = (f32::from(salary) * 0.2).ceil();
    let discounted_salary = salary - (discount_amount_rounded_up as i16);
    cmp::max(discounted_salary, 1)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use color_eyre::Result;
    use once_cell::sync::Lazy;
    use sea_orm::ActiveValue;

    use crate::contract::{
        free_agent_extension::{
            get_salary_discounted_by_10_percent, get_salary_discounted_by_20_percent,
            sign_rfa_or_ufa_contract_to_team,
        },
        ContractKind, ContractStatus, Model,
    };

    static NOW: Lazy<DateTime<FixedOffset>> = Lazy::new(|| {
        DateTime::parse_from_str("2023 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
            .unwrap()
    });

    fn generate_contract() -> Model {
        Model {
            id: 1,
            kind: ContractKind::RookieDevelopment,
            year_number: 1,
            salary: 4,
            is_ir: false,
            end_of_season_year: 2023,
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
        test_contract.kind = ContractKind::RestrictedFreeAgent;
        test_contract.salary = 4;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 11)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(4));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::RookieExtension)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(9));

        Ok(())
    }

    #[test]
    fn sign_rfa_different_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::RestrictedFreeAgent;
        test_contract.salary = 4;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 11)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(11));

        Ok(())
    }

    #[test]
    fn sign_ufa20_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentOriginalTeam;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(26));

        Ok(())
    }

    #[test]
    fn sign_ufa20_different_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentOriginalTeam;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(33));

        Ok(())
    }

    #[test]
    fn sign_ufa10_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentVeteran;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(29));

        Ok(())
    }

    #[test]
    fn sign_ufa10_new_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentVeteran;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(33));

        Ok(())
    }

    #[test]
    fn discounts_calculate_correctly() {
        assert_eq!(get_salary_discounted_by_10_percent(1), 1);
        assert_eq!(get_salary_discounted_by_10_percent(2), 1);
        assert_eq!(get_salary_discounted_by_10_percent(3), 2);
        assert_eq!(get_salary_discounted_by_10_percent(4), 3);
        assert_eq!(get_salary_discounted_by_10_percent(5), 4);
        assert_eq!(get_salary_discounted_by_10_percent(6), 5);
        assert_eq!(get_salary_discounted_by_10_percent(7), 6);
        assert_eq!(get_salary_discounted_by_10_percent(8), 7);
        assert_eq!(get_salary_discounted_by_10_percent(9), 8);
        assert_eq!(get_salary_discounted_by_10_percent(10), 9);
        assert_eq!(get_salary_discounted_by_10_percent(11), 9);
        assert_eq!(get_salary_discounted_by_10_percent(12), 10);
        assert_eq!(get_salary_discounted_by_10_percent(15), 13);
        assert_eq!(get_salary_discounted_by_10_percent(19), 17);
        assert_eq!(get_salary_discounted_by_10_percent(20), 18);
        assert_eq!(get_salary_discounted_by_10_percent(21), 18);
        assert_eq!(get_salary_discounted_by_10_percent(29), 26);
        assert_eq!(get_salary_discounted_by_10_percent(30), 27);
        assert_eq!(get_salary_discounted_by_10_percent(31), 27);

        assert_eq!(get_salary_discounted_by_20_percent(1), 1);
        assert_eq!(get_salary_discounted_by_20_percent(2), 1);
        assert_eq!(get_salary_discounted_by_20_percent(3), 2);
        assert_eq!(get_salary_discounted_by_20_percent(4), 3);
        assert_eq!(get_salary_discounted_by_20_percent(5), 4);
        assert_eq!(get_salary_discounted_by_20_percent(6), 4);
        assert_eq!(get_salary_discounted_by_20_percent(7), 5);
        assert_eq!(get_salary_discounted_by_20_percent(8), 6);
        assert_eq!(get_salary_discounted_by_20_percent(9), 7);
        assert_eq!(get_salary_discounted_by_20_percent(10), 8);
        assert_eq!(get_salary_discounted_by_20_percent(11), 8);
        assert_eq!(get_salary_discounted_by_20_percent(12), 9);
        assert_eq!(get_salary_discounted_by_20_percent(15), 12);
        assert_eq!(get_salary_discounted_by_20_percent(19), 15);
        assert_eq!(get_salary_discounted_by_20_percent(20), 16);
        assert_eq!(get_salary_discounted_by_20_percent(21), 16);
        assert_eq!(get_salary_discounted_by_20_percent(29), 23);
        assert_eq!(get_salary_discounted_by_20_percent(30), 24);
        assert_eq!(get_salary_discounted_by_20_percent(31), 24);
        assert_eq!(get_salary_discounted_by_20_percent(35), 28);
        assert_eq!(get_salary_discounted_by_20_percent(36), 28);
        assert_eq!(get_salary_discounted_by_20_percent(40), 32);
        assert_eq!(get_salary_discounted_by_20_percent(41), 32);
    }
}
