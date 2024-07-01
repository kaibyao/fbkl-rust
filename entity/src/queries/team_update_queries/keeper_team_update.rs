use color_eyre::Result;
use fbkl_constants::league_rules::KEEPER_CONTRACT_TOTAL_SALARY_LIMIT;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait};
use std::fmt::Debug;
use tracing::instrument;

use crate::{
    contract::{self, ContractKind},
    team,
    team_update::{self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData},
    transaction,
};

use super::ContractUpdatePlayerData;

static IGNORE_CONTRACT_TYPES_FOR_KEEPERS: [ContractKind; 3] = [
    ContractKind::RestrictedFreeAgent,
    ContractKind::UnrestrictedFreeAgentOriginalTeam,
    ContractKind::UnrestrictedFreeAgentVeteran,
];

#[instrument]
async fn generate_keeper_team_update_data<C>(
    team_model: &team::Model,
    keeper_contracts: &[contract::Model],
    db: &C,
) -> Result<TeamUpdateData>
where
    C: ConnectionTrait + Debug,
{
    let all_active_team_contracts = team_model.get_active_contracts(db).await?;

    let mut contract_updates = vec![];
    let mut team_contract_ids = vec![];
    let mut total_salary = 0;
    for team_contract_model in all_active_team_contracts {
        let contract_update_player_data =
            ContractUpdatePlayerData::from_contract_model(&team_contract_model, db).await?;

        if keeper_contracts.contains(&team_contract_model) {
            contract_updates.push(ContractUpdate {
                contract_id: team_contract_model.id,
                update_type: ContractUpdateType::Keeper,
                player_name_at_time: contract_update_player_data.player_name,
                player_team_abbr_at_time: contract_update_player_data.real_team_abbr,
                player_team_name_at_time: contract_update_player_data.real_team_name,
            });

            team_contract_ids.push(team_contract_model.id);
            total_salary += team_contract_model.salary;
        } else if !IGNORE_CONTRACT_TYPES_FOR_KEEPERS.contains(&team_contract_model.kind) {
            contract_updates.push(ContractUpdate {
                contract_id: team_contract_model.id,
                update_type: ContractUpdateType::Drop,
                player_name_at_time: contract_update_player_data.player_name,
                player_team_abbr_at_time: contract_update_player_data.real_team_abbr,
                player_team_name_at_time: contract_update_player_data.real_team_name,
            });
        }
    }
    let team_update_data = TeamUpdateData::from_assets(
        team_contract_ids,
        vec![TeamUpdateAsset::Contracts(contract_updates)],
        total_salary,
        KEEPER_CONTRACT_TOTAL_SALARY_LIMIT,
        0,
        0,
    );
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
        data: ActiveValue::Set(team_update_data.to_json()?),
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
    keeper_team_update_to_edit.data = ActiveValue::Set(team_update_data.to_json()?);
    let updated_model = keeper_team_update_to_edit.update(db).await?;
    Ok(updated_model)
}
