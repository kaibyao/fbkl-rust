use std::fmt::Debug;

use color_eyre::eyre::{eyre, Result};
use fbkl_entity::{
    contract, contract_queries, deadline,
    sea_orm::{ActiveValue, ConnectionTrait},
    team,
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries::{self, ContractUpdatePlayerData},
    transaction::{self, TransactionType},
    transaction_queries,
};
use tracing::instrument;

use crate::roster::{calculate_team_contract_salary, calculate_team_contract_salary_with_model};

#[instrument]
pub async fn move_contract_to_ir<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for an IR contract with id: {}",
            contract_model.id
        )
    })?;
    let (original_salary, original_salary_cap) =
        calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;

    let updated_contract = contract_queries::move_contract_to_ir(contract_model, db).await?;

    // create transaction
    let ir_transaction_to_insert = transaction::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(updated_contract.end_of_season_year),
        transaction_type: ActiveValue::Set(TransactionType::TeamUpdateToIr),
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
        original_salary,
        original_salary_cap,
        ir_transaction.id,
        db,
    )
    .await?;

    Ok(updated_contract)
}

async fn create_ir_team_update<C>(
    ir_contract_model: &contract::Model,
    deadline_model: &deadline::Model,
    team_model: &team::Model,
    original_salary: i16,
    original_salary_cap: i16,
    ir_transaction_id: i64,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let team_active_contracts = team_model.get_active_contracts(db).await?;
    let related_player_data =
        ContractUpdatePlayerData::from_contract_model(ir_contract_model, db).await?;
    let (new_salary, new_salary_cap) =
        calculate_team_contract_salary(team_model.id, &team_active_contracts, deadline_model, db)
            .await?;

    let team_update_data = TeamUpdateData::from_assets(
        team_active_contracts.iter().map(|c| c.id).collect(),
        vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
            contract_id: ir_contract_model.id,
            update_type: ContractUpdateType::ToIR,
            player_name_at_time: related_player_data.player_name,
            player_team_abbr_at_time: related_player_data.real_team_abbr,
            player_team_name_at_time: related_player_data.real_team_name,
        }])],
        new_salary,
        new_salary_cap,
        original_salary,
        original_salary_cap,
    );

    let team_update_to_insert = team_update::ActiveModel {
        data: ActiveValue::Set(team_update_data.to_json()?),
        effective_date: ActiveValue::Set(deadline_model.date_time.date_naive()),
        status: ActiveValue::Set(TeamUpdateStatus::Pending),
        team_id: ActiveValue::Set(team_model.id),
        transaction_id: ActiveValue::Set(Some(ir_transaction_id)),
        ..Default::default()
    };
    let inserted_team_update =
        team_update_queries::insert_team_update(team_update_to_insert, db).await?;

    Ok(inserted_team_update)
}
