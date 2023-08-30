use std::fmt::Debug;

use color_eyre::eyre::{bail, ensure, Result};
use fbkl_constants::league_rules::{
    POST_SEASON_DEFAULT_TOTAL_SALARY_LIMIT, PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_DEFAULT_TOTAL_SALARY_LIMIT,
    REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT,
};
use fbkl_entity::{
    contract::{self, ContractType},
    contract_queries,
    deadline::{self, DeadlineType},
    deadline_queries,
    sea_orm::ConnectionTrait,
    team_update::{self, ContractUpdateType, TeamUpdateAsset, TeamUpdateData},
    team_update_queries,
};
use multimap::MultiMap;
use once_cell::sync::Lazy;
use tracing::instrument;

static EMPTY_VEC: &Vec<team_update::Model> = &vec![];

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
    let team_updates_by_team: MultiMap<i64, team_update::Model> =
        team_update_queries::find_team_updates_for_deadline(roster_lock_deadline, db)
            .await?
            .into_iter()
            .map(|team_update_model| (team_update_model.team_id, team_update_model))
            .collect();

    for (team_id, team_contracts) in league_contracts_by_team.iter_all() {
        let team_updates = team_updates_by_team.get_vec(team_id).unwrap_or(EMPTY_VEC);
        validate_roster_ir_slot_limits(team_contracts)?;
        validate_roster_contract_type_limits_not_exceeded(team_contracts, roster_lock_deadline)?;
        validate_roster_cap_not_exceeded(team_contracts, team_updates, roster_lock_deadline, db)
            .await?;
    }

    Ok(())
}

static CONTRACT_TYPES_COUNTED_TOWARD_CAP: Lazy<&[ContractType]> = Lazy::new(|| {
    &[
        ContractType::Rookie,
        ContractType::RookieExtension,
        ContractType::Veteran,
    ]
});

async fn validate_roster_cap_not_exceeded<C>(
    team_contracts: &[contract::Model],
    team_updates: &[team_update::Model],
    roster_lock_deadline: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let contracts_counted_towards_cap: Vec<&contract::Model> = team_contracts
        .iter()
        .filter(|contract_model| {
            CONTRACT_TYPES_COUNTED_TOWARD_CAP.contains(&contract_model.contract_type)
                && !contract_model.is_ir
        })
        .collect();
    let total_contract_amount = contracts_counted_towards_cap
        .iter()
        .fold(0, |sum, contract_model| sum + contract_model.salary);

    // Need to calculate the used cap, which could be affected by dropped contracts since the start of the season.
    let mut team_salary_cap = match roster_lock_deadline.deadline_type {
        DeadlineType::InSeasonRosterLock => {
            let fa_auction_end_deadline = deadline_queries::find_deadline_for_season_by_type(
                roster_lock_deadline.league_id,
                roster_lock_deadline.end_of_season_year,
                DeadlineType::FreeAgentAuctionEnd,
                db,
            )
            .await?;
            if roster_lock_deadline.date_time > fa_auction_end_deadline.date_time {
                POST_SEASON_DEFAULT_TOTAL_SALARY_LIMIT
            } else {
                REGULAR_SEASON_DEFAULT_TOTAL_SALARY_LIMIT
            }
        }
        DeadlineType::TradeDeadlineAndPlayoffStart => POST_SEASON_DEFAULT_TOTAL_SALARY_LIMIT,
        DeadlineType::SeasonEnd => POST_SEASON_DEFAULT_TOTAL_SALARY_LIMIT,
        DeadlineType::PreseasonKeeper => {
            bail!("Not validating pre-season keeper deadline in this function.")
        }
        _ => REGULAR_SEASON_DEFAULT_TOTAL_SALARY_LIMIT,
    };

    if roster_lock_deadline.deadline_type != DeadlineType::PreseasonKeeper {
        let dropped_contract_ids = get_dropped_contract_ids(team_updates)?;
        let dropped_contracts =
            contract_queries::find_contracts_by_ids(dropped_contract_ids, db).await?;
        let dropped_contract_cap_penalty = dropped_contracts
            .iter()
            .filter(|contract_model| {
                contract_model.contract_type != ContractType::RookieDevelopment
                    && contract_model.contract_type != ContractType::RookieDevelopmentInternational
            })
            .fold(0, |sum, dropped_contract| {
                let penalty_amount_rounded_up = (f32::from(dropped_contract.salary) * 0.2).ceil();
                sum + penalty_amount_rounded_up as i16
            });

        team_salary_cap -= dropped_contract_cap_penalty;
    }

    if total_contract_amount > team_salary_cap {
        bail!("Roster contracts are invalid for roster lock: contract salaries exceed the team's cap. Deadline: {}, League: {}, End-of-season year: {}, Team: {}.", roster_lock_deadline.id, roster_lock_deadline.league_id, roster_lock_deadline.end_of_season_year, team_contracts[0].team_id.unwrap());
    }

    Ok(())
}

fn get_dropped_contract_ids(team_updates: &[team_update::Model]) -> Result<Vec<i64>> {
    let mut dropped_contract_ids = vec![];
    for team_update_model in team_updates {
        let TeamUpdateData::Assets(team_update_assets) = team_update_model.get_data()? else {
            continue
        };

        for team_update_asset in team_update_assets {
            let TeamUpdateAsset::Contracts(team_update_contracts) = team_update_asset else {
                continue
            };

            for contract_update in team_update_contracts {
                if contract_update.update_type == ContractUpdateType::Drop {
                    dropped_contract_ids.push(contract_update.contract_id);
                }
            }
        }
    }

    Ok(dropped_contract_ids)
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
