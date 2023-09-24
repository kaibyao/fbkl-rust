use std::fmt::Debug;

use color_eyre::eyre::{eyre, Result};
use fbkl_entity::{
    contract, contract_queries, deadline,
    sea_orm::{ActiveValue, ConnectionTrait},
    transaction::{self, TransactionType},
    transaction_queries,
};
use tracing::instrument;

use crate::roster::calculate_team_contract_salary_with_model;

use super::rookie_activation_team_update::create_rookie_activation_team_update;

#[instrument]
pub async fn activate_rookie_development_contract<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for a RD(I) contract with id: {}",
            contract_model.id
        )
    })?;
    let (original_salary, original_salary_cap) =
        calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;
    let activated_contract =
        contract_queries::activate_rookie_development_contract(contract_model, db).await?;

    // create transaction
    let transaction_to_insert = transaction::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(activated_contract.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::RookieContractActivation),
        league_id: ActiveValue::Set(activated_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        rookie_contract_activation_id: ActiveValue::Set(Some(activated_contract.id)),
        ..Default::default()
    };
    let inserted_transaction =
        transaction_queries::insert_transaction(transaction_to_insert, db).await?;

    // create team_update
    create_rookie_activation_team_update(
        &activated_contract,
        deadline_model,
        &team_model,
        (original_salary, original_salary_cap),
        inserted_transaction.id,
        db,
    )
    .await?;

    Ok(activated_contract)
}
