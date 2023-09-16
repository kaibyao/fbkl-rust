use std::fmt::Debug;

use color_eyre::eyre::{bail, ensure, Result};
use fbkl_constants::league_rules::{
    PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT,
};
use fbkl_entity::{
    contract::{self, ContractType},
    contract_queries,
    deadline::{self, DeadlineType},
    sea_orm::ConnectionTrait,
};
use multimap::MultiMap;
use tracing::instrument;

use crate::roster::calculate_team_contract_salary;

/// Validate if a roster is ready for a lock.
#[instrument]
pub async fn validate_league_rosters<C>(
    roster_lock_deadline: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    if roster_lock_deadline.deadline_type == DeadlineType::PreseasonKeeper {
        // Reason being that the keeper lock uses rules that are so different from the regular season that it made sense for it to have its own validation functions.
        bail!("validate_league_rosters should not be used to validate keepers. use 'save_keeper_team_update' instead.");
    }

    let league_contracts_by_team: MultiMap<i64, contract::Model> =
        contract_queries::find_active_contracts_in_league(roster_lock_deadline.league_id, db)
            .await?
            .into_iter()
            .filter_map(|contract_model| {
                contract_model
                    .team_id
                    .map(|team_id| (team_id, contract_model))
            })
            .collect();

    for (team_id, team_contracts) in league_contracts_by_team.iter_all() {
        validate_roster_ir_slot_limits(team_contracts)?;
        validate_roster_contract_type_limits_not_exceeded(team_contracts, roster_lock_deadline)?;
        validate_roster_cap_not_exceeded(*team_id, team_contracts, roster_lock_deadline, db)
            .await?;
    }

    Ok(())
}

async fn validate_roster_cap_not_exceeded<C>(
    team_id: i64,
    team_contracts: &[contract::Model],
    roster_lock_deadline: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let (total_contract_amount, team_salary_cap) =
        calculate_team_contract_salary(team_id, team_contracts, roster_lock_deadline, db).await?;

    if total_contract_amount > team_salary_cap {
        bail!("Roster contracts are invalid for roster lock: contract salaries exceed the team's cap. Deadline: {}, League: {}, End-of-season year: {}, Team: {}.", roster_lock_deadline.id, roster_lock_deadline.league_id, roster_lock_deadline.end_of_season_year, team_contracts[0].team_id.unwrap());
    }

    Ok(())
}

fn validate_roster_contract_type_limits_not_exceeded(
    team_contracts: &[contract::Model],
    roster_lock_deadline: &deadline::Model,
) -> Result<()> {
    let mut num_rd_contracts = 0;
    let mut num_rdi_contracts = 0;
    let mut num_v_r_contracts = 0;

    for contract_model in team_contracts {
        if contract_model.is_ir {
            continue;
        }

        match contract_model.contract_type {
            ContractType::RookieDevelopment => num_rd_contracts += 1,
            ContractType::RookieDevelopmentInternational => num_rdi_contracts += 1,
            ContractType::Rookie | ContractType::RookieExtension | ContractType::Veteran => {
                num_v_r_contracts += 1
            }
            _ => (),
        }
    }

    match roster_lock_deadline.deadline_type {
        DeadlineType::PreseasonKeeper => {
            bail!("Not validating pre-season keeper deadline in this function.")
        }
        DeadlineType::PreseasonStart => {
            bail!("Not validating pre-season start deadline in this function.")
        }
        DeadlineType::PreseasonVeteranAuctionStart
        | DeadlineType::PreseasonFaAuctionStart
        | DeadlineType::PreseasonFaAuctionEnd
        | DeadlineType::PreseasonRookieDraftStart => {
            if num_rd_contracts + num_rdi_contracts + num_v_r_contracts
                > PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT
            {
                bail!(
                    "Preseason roster cannot exceed {} contracts.",
                    PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT
                )
            }
        }
        DeadlineType::PreseasonFinalRosterLock
        | DeadlineType::Week1FreeAgentAuctionStart
        | DeadlineType::Week1FreeAgentAuctionEnd
        | DeadlineType::Week1RosterLock
        | DeadlineType::InSeasonRosterLock
        | DeadlineType::FreeAgentAuctionEnd
        | DeadlineType::TradeDeadlineAndPlayoffStart
        | DeadlineType::SeasonEnd => {
            if num_rd_contracts > REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT {
                bail!(
                    "Roster cannot exceed {} rookie development contracts. (team = {}).",
                    REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
                    team_contracts[0].team_id.unwrap()
                );
            }

            if num_rdi_contracts > REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT
            {
                bail!("Roster cannot have more than {} international rookie development contract. (team = {}).", REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT, team_contracts[0].team_id.unwrap());
            }

            if num_v_r_contracts > REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT {
                bail!("Roster cannot have more than {} veteran or rookie-scale contracts. (team = {}).", REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT, team_contracts[0].team_id.unwrap());
            }
        }
    };

    todo!()
}

fn validate_roster_ir_slot_limits(team_contracts: &[contract::Model]) -> Result<()> {
    let number_ir_contracts = team_contracts
        .iter()
        .filter(|contract_model| contract_model.is_ir)
        .count() as i16;
    ensure!(
        number_ir_contracts >= 0
            && number_ir_contracts <= REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT,
        "Cannot exceed {} IR contract on roster. (team = {})",
        REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT,
        team_contracts[0].team_id.unwrap()
    );
    Ok(())
}
