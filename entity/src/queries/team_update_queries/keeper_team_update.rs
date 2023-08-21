use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait};
use std::fmt::Debug;
use tracing::instrument;

use crate::{
    contract::{self, ContractType},
    contract_queries, team,
    team_update::{self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData},
    transaction,
};

use super::ContractUpdatePlayerData;

#[instrument]
async fn generate_keeper_team_update_data<C>(
    team_model: &team::Model,
    keeper_contracts: &[contract::Model],
    db: &C,
) -> Result<TeamUpdateData>
where
    C: ConnectionTrait + Debug,
{
    let all_active_team_contracts =
        contract_queries::find_active_contracts_for_team(team_model, db).await?;
    let ignore_contract_types_for_keepers = [
        ContractType::RestrictedFreeAgent,
        ContractType::UnrestrictedFreeAgentOriginalTeam,
        ContractType::UnrestrictedFreeAgentVeteran,
    ];

    let mut contract_updates = vec![];
    for team_contract_model in all_active_team_contracts {
        let contract_update_player_data =
            ContractUpdatePlayerData::from_contract_model(&team_contract_model, db).await?;

        if keeper_contracts.contains(&team_contract_model) {
            contract_updates.push(ContractUpdate {
                contract_id: team_contract_model.id,
                update_type: ContractUpdateType::Keeper,
                player_name_at_time_of_trade: contract_update_player_data.player_name,
                player_team_abbr_at_time_of_trade: contract_update_player_data.real_team_abbr,
                player_team_name_at_time_of_trade: contract_update_player_data.real_team_name,
            });
        } else if !ignore_contract_types_for_keepers.contains(&team_contract_model.contract_type) {
            contract_updates.push(ContractUpdate {
                contract_id: team_contract_model.id,
                update_type: ContractUpdateType::Drop,
                player_name_at_time_of_trade: contract_update_player_data.player_name,
                player_team_abbr_at_time_of_trade: contract_update_player_data.real_team_abbr,
                player_team_name_at_time_of_trade: contract_update_player_data.real_team_name,
            });
        }
    }
    let team_update_data =
        TeamUpdateData::Assets(vec![TeamUpdateAsset::Contracts(contract_updates)]);
    Ok(team_update_data)
}

/// Inserts & returns a new team update containing keeper contracts for a specific team.
#[instrument]
pub async fn insert_keeper_team_update<C>(
    team_model: &team::Model,
    keeper_contracts: &[contract::Model],
    keeper_transaction: &transaction::Model,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let team_update_data =
        generate_keeper_team_update_data(team_model, keeper_contracts, db).await?;

    let team_update_to_insert = team_update::ActiveModel {
        data: ActiveValue::Set(team_update_data.as_bytes()?),
        effective_date: ActiveValue::Set(
            keeper_transaction
                .get_deadline(db)
                .await?
                .date_time
                .date_naive(),
        ),
        status: ActiveValue::Set(team_update::TeamUpdateStatus::Pending),
        team_id: ActiveValue::Set(team_model.id),
        transaction_id: ActiveValue::Set(Some(keeper_transaction.id)),
        ..Default::default()
    };
    let team_update = team_update_to_insert.insert(db).await?;

    Ok(team_update)
}

#[instrument]
pub async fn update_keeper_team_update<C>(
    team_model: &team::Model,
    keeper_team_update: team_update::Model,
    keeper_contracts: &[contract::Model],
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut keeper_team_update_to_edit: team_update::ActiveModel = keeper_team_update.into();
    let team_update_data =
        generate_keeper_team_update_data(team_model, keeper_contracts, db).await?;
    keeper_team_update_to_edit.data = ActiveValue::Set(team_update_data.as_bytes()?);
    let updated_model = keeper_team_update_to_edit.update(db).await?;
    Ok(updated_model)
}
