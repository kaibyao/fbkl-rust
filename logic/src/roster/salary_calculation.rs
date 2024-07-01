use std::fmt::Debug;

use color_eyre::eyre::Result;
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    deadline::{self, DeadlineKind},
    sea_orm::ConnectionTrait,
    team,
};
use once_cell::sync::Lazy;
use tracing::instrument;

static CONTRACT_TYPES_COUNTED_TOWARD_CAP: Lazy<&[ContractKind]> = Lazy::new(|| {
    &[
        ContractKind::Rookie,
        ContractKind::RookieExtension,
        ContractKind::Veteran,
    ]
});

/// Returns a tuple containing the team's current total salary and salary cap.
#[instrument]
pub async fn calculate_team_contract_salary_with_model<C>(
    team_model: &team::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<(i16, i16)>
where
    C: ConnectionTrait + Debug,
{
    let team_active_contracts = team_model.get_active_contracts(db).await?;

    calculate_team_contract_salary(team_model.id, &team_active_contracts, deadline_model, db).await
}

/// Returns a tuple containing the team's current total salary and salary cap.
#[instrument]
pub async fn calculate_team_contract_salary<C>(
    team_id: i64,
    team_active_contracts: &[contract::Model],
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<(i16, i16)>
where
    C: ConnectionTrait + Debug,
{
    let max_salary_cap_for_deadline = deadline_model.get_salary_cap(db).await?;

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
        return Ok((total_contract_amount, max_salary_cap_for_deadline));
    }

    let dropped_team_contracts =
        contract_queries::find_contracts_dropped_by_team_in_regular_season(
            team_id,
            deadline_model.end_of_season_year,
            db,
        )
        .await?;
    let dropped_contract_cap_penalty = dropped_team_contracts
        .iter()
        .filter(|contract_model| {
            CONTRACT_TYPES_COUNTED_TOWARD_CAP.contains(&contract_model.kind)
        })
        .fold(0, |sum, dropped_contract| {
            let penalty_amount_rounded_up = (f32::from(dropped_contract.salary) * 0.2).ceil();
            sum + penalty_amount_rounded_up as i16
        });
    let team_salary_cap = max_salary_cap_for_deadline - dropped_contract_cap_penalty;

    Ok((total_contract_amount, team_salary_cap))
}
