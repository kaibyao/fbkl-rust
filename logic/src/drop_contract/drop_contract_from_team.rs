use std::fmt::Debug;

use color_eyre::eyre::{ensure, eyre, Result};
use fbkl_entity::{
    contract::{self, ContractStatus},
    contract_queries,
    deadline::{self, DeadlineType},
    sea_orm::{ActiveValue, ConnectionTrait},
    transaction::{self, TransactionType},
    transaction_queries,
};
use tracing::instrument;

use crate::roster::calculate_team_contract_salary_with_model;

use super::drop_contract_team_update::create_drop_contract_team_update;

#[instrument]
pub async fn drop_contract_from_team<C>(
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
            "Could not retrieve the expected team for a contract intended to be dropped (id = {})",
            contract_model.id
        )
    })?;
    let (original_salary, original_salary_cap) =
        calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;

    let is_drop_before_keeper_deadline = is_deadline_before_preseason_keeper(deadline_model);
    let dropped_contract =
        contract_queries::drop_contract(contract_model, is_drop_before_keeper_deadline, db).await?;

    // create transaction
    let transaction_to_insert = transaction::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(dropped_contract.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::TeamUpdateDropContract),
        league_id: ActiveValue::Set(dropped_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        dropped_contract_id: ActiveValue::Set(Some(dropped_contract.id)),
        ..Default::default()
    };
    let transaction_model =
        transaction_queries::insert_transaction(transaction_to_insert, db).await?;

    // create team_update
    create_drop_contract_team_update(
        &dropped_contract,
        deadline_model,
        &team_model,
        (original_salary, original_salary_cap),
        transaction_model.id,
        db,
    )
    .await?;

    Ok(dropped_contract)
}

fn validate_contract_eligibility(contract_model: &contract::Model) -> Result<()> {
    ensure!(
        contract_model.status == ContractStatus::Active,
        "Cannot drop a contract that's not active. (contract_id = {})",
        contract_model.id
    );
    Ok(())
}

fn is_deadline_before_preseason_keeper(deadline_model: &deadline::Model) -> bool {
    [DeadlineType::PreseasonStart, DeadlineType::PreseasonKeeper]
        .contains(&deadline_model.deadline_type)
}
