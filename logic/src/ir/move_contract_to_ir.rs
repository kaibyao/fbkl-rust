use color_eyre::eyre::{Result, eyre};
use fbkl_entity::{
    contract, contract_queries, deadline,
    league_event::{self, LeagueEventKind},
    league_event_queries,
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::ContractUpdateType,
};
use tracing::instrument;

use crate::roster::{
    RosterMoveRejection, SalarySnapshot, calculate_team_contract_salary_with_model,
};

use super::ir_team_update::create_ir_team_update;

#[instrument(skip(db))]
pub async fn move_contract_to_ir<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    // Rules §10.3.2. Rule §10.3.1 (an in-season acquisition has to be accommodated on the 22-man
    // active roster before it goes to IR) is T2 in `roster::validate_transaction`, which the caller
    // submitting the transaction runs: the ban lasts for the transaction that acquired the contract,
    // and a lone move to IR is judged by roster legality alone (rules §13.1.6).
    if contract_model.is_ir {
        return Err(RosterMoveRejection::AlreadyInIr {
            contract_id: contract_model.id,
        }
        .into());
    }

    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for an contract with id: {}",
            contract_model.id
        )
    })?;
    let SalarySnapshot {
        salary: original_salary,
        cap: original_salary_cap,
    } = calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;

    let updated_contract = contract_queries::move_contract_to_ir(contract_model, db).await?;

    // create league event
    let ir_league_event_to_insert = league_event::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(updated_contract.end_of_season_year),
        kind: ActiveValue::Set(LeagueEventKind::TeamUpdateToIr),
        league_id: ActiveValue::Set(updated_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        contract_id: ActiveValue::Set(Some(updated_contract.id)),
        ..Default::default()
    };
    let ir_league_event =
        league_event_queries::insert_league_event(ir_league_event_to_insert, db).await?;

    // create team_update
    create_ir_team_update(
        &updated_contract,
        deadline_model,
        &team_model,
        ContractUpdateType::ToIR,
        (original_salary, original_salary_cap),
        ir_league_event.id,
        db,
    )
    .await?;

    Ok(updated_contract)
}
