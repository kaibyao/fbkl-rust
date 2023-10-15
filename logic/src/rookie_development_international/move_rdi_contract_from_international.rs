use std::fmt::Debug;

use color_eyre::eyre::{eyre, Result};
use fbkl_entity::{
    contract, contract_queries, deadline,
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::ContractUpdateType,
    transaction::{self, TransactionType},
    transaction_queries,
};
use tracing::instrument;

use super::rdi_team_update::create_rdi_move_team_update;

#[instrument]
pub async fn move_rookie_development_international_contract_to_stateside<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for an RD contract with id: {}",
            contract_model.id
        )
    })?;
    let moved_contract = contract_queries::move_rdi_contract_to_rd(contract_model, db).await?;

    // create transaction
    let transaction_to_insert = transaction::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(moved_contract.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::TeamUpdateFromRdi),
        league_id: ActiveValue::Set(moved_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        rdi_contract_id: ActiveValue::Set(Some(moved_contract.id)),
        ..Default::default()
    };
    let inserted_transaction =
        transaction_queries::insert_transaction(transaction_to_insert, db).await?;

    // create team_update
    create_rdi_move_team_update(
        &moved_contract,
        deadline_model,
        &team_model,
        ContractUpdateType::FromRdi,
        inserted_transaction.id,
        db,
    )
    .await?;

    Ok(moved_contract)
}
