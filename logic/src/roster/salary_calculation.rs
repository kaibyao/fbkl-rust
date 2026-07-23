use std::fmt::Debug;

use chrono::{DateTime, FixedOffset};
use color_eyre::eyre::Result;
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries::{self, find_active_contracts_for_team},
    deadline::{self, DeadlineKind},
    deadline_queries::find_most_recent_deadline_by_datetime,
    sea_orm::ConnectionTrait,
    team,
};
use tracing::instrument;

static CONTRACT_TYPES_COUNTED_TOWARD_CAP: [ContractKind; 3] = [
    ContractKind::Rookie,
    ContractKind::RookieExtension,
    ContractKind::Veteran,
];

/// A team's total counted salary and its salary cap at a given deadline.
#[derive(Debug, Clone, Copy)]
pub struct SalarySnapshot {
    pub salary: i16,
    pub cap: i16,
}

#[instrument]
pub async fn calculate_team_contract_salary_with_model<C>(
    team_model: &team::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<SalarySnapshot>
where
    C: ConnectionTrait + Debug,
{
    let team_active_contracts = team_model.get_active_contracts(db).await?;

    calculate_team_contract_salary(team_model.id, &team_active_contracts, deadline_model, db).await
}

#[instrument]
pub async fn calculate_team_contract_salary_at_datetime<C>(
    league_id: i64,
    team_id: i64,
    datetime: DateTime<FixedOffset>,
    db: &C,
) -> Result<SalarySnapshot>
where
    C: ConnectionTrait + Debug,
{
    let deadline = find_most_recent_deadline_by_datetime(league_id, datetime, db).await?;
    let contract_models = find_active_contracts_for_team(team_id, db).await?;

    calculate_team_contract_salary(team_id, &contract_models, &deadline, db).await
}

#[instrument]
pub async fn calculate_team_contract_salary<C>(
    team_id: i64,
    team_active_contracts: &[contract::Model],
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<SalarySnapshot>
where
    C: ConnectionTrait + Debug,
{
    // `None` = §4.2.4 uncapped window (PreseasonStart → keeper deadline); i16::MAX makes cap comparisons trivially pass.
    let max_salary_cap_for_deadline = deadline_model.get_salary_cap(db).await?.unwrap_or(i16::MAX);

    let contracts_counted_towards_cap: Vec<&contract::Model> = team_active_contracts
        .iter()
        .filter(|contract_model| {
            CONTRACT_TYPES_COUNTED_TOWARD_CAP.contains(&contract_model.kind)
                && !contract_model.is_ir
        })
        .collect();
    let total_contract_amount = contracts_counted_towards_cap
        .iter()
        .fold(0, |sum, contract_model| sum + contract_model.salary);

    if deadline_model.kind == DeadlineKind::PreseasonKeeper {
        return Ok(SalarySnapshot {
            salary: total_contract_amount,
            cap: max_salary_cap_for_deadline,
        });
    }

    let dropped_team_contracts =
        contract_queries::find_contracts_dropped_by_team_in_regular_season(
            team_id,
            deadline_model.end_of_season_year,
            db,
        )
        .await?;
    // salaries are far below i16::MAX, so the rounded penalty never truncates
    #[allow(clippy::cast_possible_truncation)]
    let dropped_contract_cap_penalty = dropped_team_contracts
        .iter()
        .filter(|contract_model| CONTRACT_TYPES_COUNTED_TOWARD_CAP.contains(&contract_model.kind))
        .fold(0, |sum, dropped_contract| {
            let penalty_amount_rounded_up = (f32::from(dropped_contract.salary) * 0.2).ceil();
            sum + penalty_amount_rounded_up as i16
        });
    let team_salary_cap = max_salary_cap_for_deadline - dropped_contract_cap_penalty;

    Ok(SalarySnapshot {
        salary: total_contract_amount,
        cap: team_salary_cap,
    })
}
