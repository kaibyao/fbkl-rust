use std::fmt::Debug;

use color_eyre::eyre::{Result, bail};
use fbkl_constants::league_rules::{
    PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
    REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT,
};
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    deadline::{self, DeadlineKind},
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
    if roster_lock_deadline.kind == DeadlineKind::PreseasonKeeper {
        // Reason being that the keeper lock uses rules that are so different from the regular season that it made sense for it to have its own validation functions.
        bail!(
            "validate_league_rosters should not be used to validate keepers. use 'save_keeper_team_update' instead."
        );
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
        validate_roster_ir_slot_limits(team_contracts, db).await?;
        validate_roster_contract_type_limits_not_exceeded(team_contracts, roster_lock_deadline, db)
            .await?;
        validate_roster_cap_not_exceeded(*team_id, team_contracts, roster_lock_deadline, db)
            .await?;
    }

    Ok(())
}

/// Formats a list of team contracts into a readable, newline-separated list for
/// inclusion in error messages. Each line is `{player_name} ${value}/{year}/{kind}`,
/// sorted by contract kind.
async fn format_team_contracts<C>(team_contracts: &[contract::Model], db: &C) -> Result<String>
where
    C: ConnectionTrait + Debug,
{
    let mut contract_lines = Vec::with_capacity(team_contracts.len());
    for c in team_contracts {
        let player = c.get_player(db).await?;
        contract_lines.push((
            c.kind,
            format!(
                "{} ${}/{}/{:?}",
                player.get_name(),
                c.salary,
                c.year_number,
                c.kind
            ),
        ));
    }
    contract_lines.sort_by_key(|(kind, _)| *kind);
    Ok(contract_lines
        .into_iter()
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n"))
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
        let contracts_str = format_team_contracts(team_contracts, db).await?;
        bail!(
            "Roster contracts are invalid for roster lock: contract salaries exceed the team's cap. Deadline: {}, League: {}, End-of-season year: {}, Team: {}. Salary/Cap: {}/{}. Contracts:\n{}",
            roster_lock_deadline.id,
            roster_lock_deadline.league_id,
            roster_lock_deadline.end_of_season_year,
            team_id,
            total_contract_amount,
            team_salary_cap,
            contracts_str
        );
    }

    Ok(())
}

async fn validate_roster_contract_type_limits_not_exceeded<C>(
    team_contracts: &[contract::Model],
    roster_lock_deadline: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let mut num_rd_contracts = 0;
    let mut num_intl_rd_contracts = 0;
    let mut num_v_r_contracts = 0;

    for contract_model in team_contracts {
        if contract_model.is_ir {
            continue;
        }

        match contract_model.kind {
            ContractKind::RookieDevelopment => num_rd_contracts += 1,
            ContractKind::RookieDevelopmentInternational => num_intl_rd_contracts += 1,
            ContractKind::Rookie | ContractKind::RookieExtension | ContractKind::Veteran => {
                num_v_r_contracts += 1;
            }
            _ => (),
        }
    }

    match roster_lock_deadline.kind {
        DeadlineKind::PreseasonKeeper => {
            bail!("Not validating pre-season keeper deadline in this function.")
        }
        DeadlineKind::PreseasonStart => {
            bail!("Not validating pre-season start deadline in this function.")
        }
        DeadlineKind::PreseasonVeteranAuctionStart
        | DeadlineKind::PreseasonFaAuctionStart
        | DeadlineKind::PreseasonFaAuctionEnd
        | DeadlineKind::PreseasonRookieDraftStart => {
            if num_rd_contracts + num_intl_rd_contracts + num_v_r_contracts
                > PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT
            {
                bail!(
                    "Preseason roster cannot exceed {} contracts.",
                    PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT
                )
            }
        }
        DeadlineKind::PreseasonFinalRosterLock
        | DeadlineKind::Week1FreeAgentAuctionStart
        | DeadlineKind::Week1FreeAgentAuctionEnd
        | DeadlineKind::Week1RosterLock
        | DeadlineKind::InSeasonRosterLock
        | DeadlineKind::FreeAgentAuctionEnd
        | DeadlineKind::TradeDeadlineAndPlayoffStart
        | DeadlineKind::SeasonEnd => {
            if num_rd_contracts > REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT {
                bail!(
                    "Roster cannot exceed {} rookie development contracts. (team = {}). Contracts:\n{}",
                    REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
                    team_contracts[0].team_id.unwrap(),
                    format_team_contracts(team_contracts, db).await?
                );
            }

            if num_intl_rd_contracts
                > REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT
            {
                bail!(
                    "Roster cannot have more than {} international rookie development contract. (team = {}). Contracts:\n{}",
                    REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT,
                    team_contracts[0].team_id.unwrap(),
                    format_team_contracts(team_contracts, db).await?
                );
            }

            if num_v_r_contracts > REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT {
                bail!(
                    "Roster cannot have more than {} veteran or rookie-scale contracts. (team = {}). Contracts:\n{}",
                    REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT,
                    team_contracts[0].team_id.unwrap(),
                    format_team_contracts(team_contracts, db).await?
                );
            }
        }
    }

    Ok(())
}

async fn validate_roster_ir_slot_limits<C>(team_contracts: &[contract::Model], db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    // roster IR counts are far below i16::MAX
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let number_ir_contracts = team_contracts
        .iter()
        .filter(|contract_model| contract_model.is_ir)
        .count() as i16;
    if !(0..=REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT).contains(&number_ir_contracts) {
        bail!(
            "Cannot exceed {} IR contract on roster. (team = {}). Contracts:\n{}",
            REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT,
            team_contracts[0].team_id.unwrap(),
            format_team_contracts(team_contracts, db).await?
        );
    }
    Ok(())
}
