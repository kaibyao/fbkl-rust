use crate::contract;
use color_eyre::{eyre::bail, Result};
use sea_orm::ActiveValue;

use super::{ContractStatus, ContractType};

/// Creates a new contract from the given one, where that contract is dropped from its team for the current season.
pub fn create_dropped_contract(
    current_contract: &contract::Model,
    is_before_pre_season_keeper_deadline: bool,
) -> Result<contract::ActiveModel> {
    let mut new_salary_for_active_players_after_drop = current_contract.salary;
    let mut new_status_for_active_players_after_drop = ContractStatus::Active;
    if is_before_pre_season_keeper_deadline {
        new_salary_for_active_players_after_drop = 1;
        new_status_for_active_players_after_drop = ContractStatus::Expired;
    }

    let new_contract = contract::ActiveModel {
        id: ActiveValue::NotSet,
        contract_year: ActiveValue::Set(1),
        contract_type: ActiveValue::Set(ContractType::FreeAgent),
        is_ir: ActiveValue::Set(current_contract.is_ir),
        salary: ActiveValue::Set(match current_contract.contract_type {
            ContractType::RookieDevelopment => 1,
            ContractType::RookieDevelopmentInternational => 1,
            ContractType::Rookie => new_salary_for_active_players_after_drop,
            ContractType::RestrictedFreeAgent => 1,
            ContractType::RookieExtension => new_salary_for_active_players_after_drop,
            ContractType::UnrestrictedFreeAgentOriginalTeam => 1,
            ContractType::Veteran => new_salary_for_active_players_after_drop,
            ContractType::UnrestrictedFreeAgentVeteran => 1,
            ContractType::FreeAgent => bail!("Impossible combination: dropping a free agent."),
        }),
        season_end_year: ActiveValue::Set(current_contract.season_end_year),
        status: ActiveValue::Set(match current_contract.contract_type {
            ContractType::RookieDevelopment => ContractStatus::Expired,
            ContractType::RookieDevelopmentInternational => ContractStatus::Expired,
            ContractType::Rookie => new_status_for_active_players_after_drop,
            ContractType::RestrictedFreeAgent => ContractStatus::Expired,
            ContractType::RookieExtension => new_status_for_active_players_after_drop,
            ContractType::UnrestrictedFreeAgentOriginalTeam => ContractStatus::Expired,
            ContractType::Veteran => new_status_for_active_players_after_drop,
            ContractType::UnrestrictedFreeAgentVeteran => ContractStatus::Expired,
            ContractType::FreeAgent => bail!("Impossible combination: dropping a free agent."),
        }),
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
        self, drop_contract::create_dropped_contract, ContractStatus, ContractType,
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
            team_id: 1,
            status: ContractStatus::Active,
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
        }
    }

    #[test]
    fn drop_rd() -> Result<()> {
        let test_contract = generate_contract();

        let advanced_contract = create_dropped_contract(&test_contract, false)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Expired)
        );

        Ok(())
    }

    #[test]
    fn drop_rfa() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::RestrictedFreeAgent;
        test_contract.salary = 4;

        let advanced_contract = create_dropped_contract(&test_contract, false)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Expired)
        );

        Ok(())
    }

    #[test]
    fn drop_r() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::RookieExtension;
        test_contract.contract_year = 5;
        test_contract.salary = 14;

        let advanced_contract = create_dropped_contract(&test_contract, false)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(14));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Active)
        );

        Ok(())
    }

    #[test]
    fn drop_r1_before_pre_season_keeper_deadline() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Rookie;
        test_contract.contract_year = 1;
        test_contract.salary = 4;

        let advanced_contract = create_dropped_contract(&test_contract, true)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Expired)
        );

        Ok(())
    }

    #[test]
    fn drop_r5_before_pre_season_keeper_deadline() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::RookieExtension;
        test_contract.contract_year = 5;
        test_contract.salary = 14;

        let advanced_contract = create_dropped_contract(&test_contract, true)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Expired)
        );

        Ok(())
    }

    #[test]
    fn drop_ufa20() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::UnrestrictedFreeAgentOriginalTeam;
        test_contract.salary = 14;

        let advanced_contract = create_dropped_contract(&test_contract, false)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Expired)
        );

        Ok(())
    }

    #[test]
    fn drop_v() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Veteran;
        test_contract.contract_year = 3;
        test_contract.salary = 30;

        let advanced_contract = create_dropped_contract(&test_contract, false)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(30));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Active)
        );

        Ok(())
    }

    #[test]
    fn drop_v_before_pre_season_keeper_deadline() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::Veteran;
        test_contract.contract_year = 3;
        test_contract.salary = 30;

        let advanced_contract = create_dropped_contract(&test_contract, true)?;
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Expired)
        );

        Ok(())
    }

    #[test]
    fn drop_ufa10() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.contract_type = ContractType::UnrestrictedFreeAgentVeteran;
        test_contract.salary = 4;

        let advanced_contract = create_dropped_contract(&test_contract, false)?;
        assert_eq!(advanced_contract.contract_year, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.contract_type,
            ActiveValue::Set(ContractType::FreeAgent)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.status,
            ActiveValue::Set(ContractStatus::Expired)
        );

        Ok(())
    }
}
