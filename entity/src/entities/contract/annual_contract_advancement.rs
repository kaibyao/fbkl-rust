use color_eyre::{eyre::bail, Result};
use rust_decimal::{Decimal, RoundingStrategy};
use rust_decimal_macros::dec;
use sea_orm::ActiveValue;

use super::{contract_entity, ContractType};

/// Creates the next year's contract from the current contract. This should be used in tandem with contract_queries::advance_contract, as we also need to update the current contract to point to the new one, plus handle various cases around RFAs/UFAs, and salaries.
pub fn create_advancement_for_contract(
    current_contract: &contract_entity::Model,
) -> Result<contract_entity::ActiveModel> {
    let mut new_contract = contract_entity::ActiveModel {
        id: ActiveValue::NotSet,
        contract_year: ActiveValue::Set(current_contract.contract_year),
        contract_type: ActiveValue::Set(current_contract.contract_type),
        is_ir: ActiveValue::Set(false),
        league_player_id: ActiveValue::Set(current_contract.league_player_id),
        salary: ActiveValue::Set(current_contract.salary),
        season_end_year: ActiveValue::Set(current_contract.season_end_year + 1),
        status: ActiveValue::Set(current_contract.status),
        league_id: ActiveValue::Set(current_contract.league_id),
        player_id: ActiveValue::Set(current_contract.player_id),
        previous_contract_id: ActiveValue::Set(Some(current_contract.id)),
        original_contract_id: ActiveValue::Set(current_contract.original_contract_id),
        team_id: ActiveValue::Set(current_contract.team_id),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    match current_contract.contract_type {
        ContractType::RookieDevelopment | ContractType::RookieDevelopmentInternational => match current_contract.contract_year {
            1 => {
                new_contract.contract_year = ActiveValue::Set(2);
            },
            2 => {
                new_contract.contract_year = ActiveValue::Set(3);
            },
            3 => {
                new_contract.contract_year = ActiveValue::Set(2);
                new_contract.contract_type = ActiveValue::Set(ContractType::Rookie);
            },
            _ => {
                bail!("Invalid year for contract type: ({:?}, {})", current_contract.contract_type, current_contract.contract_year);
            }
        },
        ContractType::Rookie => match current_contract.contract_year {
                1 => {
                    new_contract.contract_year = ActiveValue::Set(2);
                },
                2 => {
                    new_contract.contract_year = ActiveValue::Set(3);
                },
                3 => {
                    new_contract.contract_type = ActiveValue::Set(ContractType::RestrictedFreeAgent);
                },
                _ => {
                    bail!("Invalid year for contract type: ({:?}, {})", current_contract.contract_type, current_contract.contract_year);
                }
            },
        ContractType::RestrictedFreeAgent => bail!("An RFA contract cannot be advanced; the contract must be either dropped or signed to a team."),
        ContractType::RookieExtension =>
            match current_contract.contract_year {
                4 => {
                    new_contract.contract_year = ActiveValue::Set(5);
                },
                5 => {
                    new_contract.contract_year = ActiveValue::Set(1);
                    new_contract.contract_type = ActiveValue::Set(ContractType::UnrestrictedFreeAgentOriginalTeam);
                },
                _ => {
                    bail!("Invalid year for contract type: ({:?}, {})", current_contract.contract_type, current_contract.contract_year);
                }
            }
        ,
        ContractType::Veteran => {
            match current_contract.contract_year {
                1 => {
                    new_contract.contract_year = ActiveValue::Set(2);
                },
                2 => {
                    new_contract.contract_year = ActiveValue::Set(3);
                },
                3 => {
                    new_contract.contract_year = ActiveValue::Set(1);
                    new_contract.contract_type = ActiveValue::Set(ContractType::UnrestrictedFreeAgentVeteran);
                },
                _ => {
                    bail!("Invalid year for contract type: ({:?}, {})", current_contract.contract_type, current_contract.contract_year);
                }
            }
        },
        ContractType::FreeAgent => bail!("Cannot advance a free agent contract."),
        ContractType::UnrestrictedFreeAgentOriginalTeam => bail!("A UFA contract cannot be advanced; the contract must be either dropped or signed to a team."),
        ContractType::UnrestrictedFreeAgentVeteran => bail!("A UFA contract cannot be advanced; the contract must be either dropped or signed to a team."),
    }

    new_contract.salary = ActiveValue::Set(calculate_yearly_salary_increase(current_contract)?);

    Ok(new_contract)
}

fn calculate_yearly_salary_increase(current_contract: &contract_entity::Model) -> Result<i16> {
    match current_contract.contract_type {
        ContractType::RookieDevelopment | ContractType::RookieDevelopmentInternational => {
            Ok(current_contract.salary)
        }
        ContractType::Rookie => Ok(get_salary_increased_by_20_percent(current_contract.salary)),
        ContractType::RestrictedFreeAgent => {
            bail!("Cannot calculate the yearly increase of an RFA contract.")
        }
        ContractType::RookieExtension => match current_contract.contract_year {
            4 => Ok(get_salary_increased_by_20_percent(current_contract.salary)),
            _ => Ok(1), // Needs to later be set during veteran auction salary fetch
        },
        ContractType::UnrestrictedFreeAgentOriginalTeam => {
            bail!("Cannot calculate the yearly increase of an RFA contract.")
        }
        ContractType::Veteran => match current_contract.contract_year {
            1 | 2 => Ok(get_salary_increased_by_20_percent(current_contract.salary)),
            _ => Ok(1), // Needs to later be set during veteran auction salary fetch
        },
        ContractType::UnrestrictedFreeAgentVeteran => {
            bail!("Cannot calculate the yearly increase of a UFA contract.")
        }
        ContractType::FreeAgent => {
            bail!("Cannot calculate the yearly increase of a free agent contract.")
        }
    }
}

fn get_salary_increased_by_20_percent(salary: i16) -> i16 {
    let salary_dec = Decimal::new(salary as i64, 0);
    let increased_salary = salary_dec * dec!(1.2);
    let rounded_up = increased_salary.round_dp_with_strategy(0, RoundingStrategy::AwayFromZero);
    rounded_up.to_string().parse().unwrap()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use color_eyre::Result;
    use once_cell::sync::Lazy;
    use sea_orm::ActiveValue;

    use crate::contract::{
        annual_contract_advancement::create_advancement_for_contract, ContractStatus, ContractType,
        Model,
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
            team_id: 1,
            status: ContractStatus::Active,
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
        }
    }

    #[test]
    fn contract_advancement_rd1_to_rd2() -> Result<()> {
        // Test RD1 -> RD2
        let test_contract = generate_contract();
        let advanced_contract = create_advancement_for_contract(&test_contract)?;

        assert_eq!(
            advanced_contract.original_contract_id,
            ActiveValue::Set(test_contract.original_contract_id)
        );
        assert_eq!(
            advanced_contract.previous_contract_id,
            ActiveValue::Set(Some(test_contract.id))
        );
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(2));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::RookieDevelopment)
        );
        assert_eq!(
            advanced_contract.salary,
            ActiveValue::Set(test_contract.salary)
        );

        Ok(())
    }

    #[test]
    fn contract_advancement_rd2_to_rd3() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.previous_contract_id = Some(1);
        test_contract.id = 2;
        test_contract.contract_year = 2;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(
            advanced_contract.original_contract_id,
            ActiveValue::Set(test_contract.original_contract_id)
        );
        assert_eq!(
            advanced_contract.previous_contract_id,
            ActiveValue::Set(Some(test_contract.id))
        );
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(3));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::RookieDevelopment)
        );
        assert_eq!(
            advanced_contract.salary,
            ActiveValue::Set(test_contract.salary)
        );

        Ok(())
    }

    #[test]
    fn contract_advancement_rd3_to_r2() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_year = 3;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(2));
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
    fn contract_advancement_r1_to_r2() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Rookie;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(2));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Rookie)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(5));

        Ok(())
    }

    #[test]
    fn contract_advancement_r2_to_r3() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Rookie;
        test_contract.contract_year = 2;
        test_contract.salary = 2;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(3));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Rookie)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(3));

        Ok(())
    }

    #[test]
    fn contract_advancement_r3_to_rfa() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Rookie;
        test_contract.contract_year = 3;
        test_contract.salary = 3;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::RestrictedFreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(4));

        Ok(())
    }

    #[test]
    fn contract_advancement_r4_to_r5() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::RookieExtension;
        test_contract.contract_year = 4;
        test_contract.salary = 11;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(5));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::RookieExtension)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(14));

        Ok(())
    }

    #[test]
    fn contract_advancement_r5_to_ufa20() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::RookieExtension;
        test_contract.contract_year = 5;
        test_contract.salary = 14;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::UnrestrictedFreeAgentOriginalTeam)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));

        Ok(())
    }

    #[test]
    fn contract_advancement_v1_to_v2() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Veteran;
        test_contract.contract_year = 1;
        test_contract.salary = 25;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(2));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(30));

        Ok(())
    }

    #[test]
    fn contract_advancement_v2_to_v3() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Veteran;
        test_contract.contract_year = 2;
        test_contract.salary = 36;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(3));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(44));

        Ok(())
    }

    #[test]
    fn contract_advancement_v3_to_ufa10() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Veteran;
        test_contract.contract_year = 3;
        test_contract.salary = 44;

        let advanced_contract = create_advancement_for_contract(&test_contract)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::UnrestrictedFreeAgentVeteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));

        Ok(())
    }
}
