use std::fmt::Debug;

use color_eyre::eyre::{ensure, eyre, Result};
use fbkl_entity::{
    contract, contract_queries, deadline,
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::ContractUpdateType,
    transaction::{self, TransactionType},
    transaction_queries,
};
use tracing::instrument;

use crate::roster::calculate_team_contract_salary_with_model;

use super::ir_team_update::create_ir_team_update;

#[instrument]
pub async fn activate_contract_from_ir<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    validate_contract_eligibility(&contract_model)?;

    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for an IR contract with id: {}",
            contract_model.id
        )
    })?;
    let (original_salary, original_salary_cap) =
        calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;

    let updated_contract = contract_queries::activate_contract_from_ir(contract_model, db).await?;

    // create transaction
    let ir_transaction_to_insert = transaction::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(updated_contract.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::TeamUpdateFromIr),
        league_id: ActiveValue::Set(updated_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        ir_contract_id: ActiveValue::Set(Some(updated_contract.id)),
        ..Default::default()
    };
    let ir_transaction =
        transaction_queries::insert_transaction(ir_transaction_to_insert, db).await?;

    // create team_update
    create_ir_team_update(
        &updated_contract,
        deadline_model,
        &team_model,
        ContractUpdateType::FromIR,
        (original_salary, original_salary_cap),
        ir_transaction.id,
        db,
    )
    .await?;

    Ok(updated_contract)
}

fn validate_contract_eligibility(contract_model: &contract::Model) -> Result<()> {
    ensure!(
        contract_model.is_ir,
        "Cannot activate a contract from IR when it is not in IR. (contract_id = {})",
        contract_model.id
    );
    Ok(())
}
