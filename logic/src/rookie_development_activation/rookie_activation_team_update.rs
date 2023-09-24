use std::fmt::Debug;

use color_eyre::eyre::Result;
use fbkl_entity::{
    contract, deadline,
    sea_orm::{ActiveValue, ConnectionTrait},
    team,
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries::{self, ContractUpdatePlayerData},
};
use tracing::instrument;

use crate::roster::calculate_team_contract_salary;

#[instrument]
pub async fn create_rookie_activation_team_update<C>(
    contract_model: &contract::Model,
    deadline_model: &deadline::Model,
    team_model: &team::Model,
    (original_salary, original_salary_cap): (i16, i16),
    transaction_id: i64,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let team_active_contracts = team_model.get_active_contracts(db).await?;
    let related_player_data =
        ContractUpdatePlayerData::from_contract_model(contract_model, db).await?;
    let (new_salary, new_salary_cap) =
        calculate_team_contract_salary(team_model.id, &team_active_contracts, deadline_model, db)
            .await?;

    let team_update_data = TeamUpdateData::from_assets(
        team_active_contracts.iter().map(|c| c.id).collect(),
        vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
            contract_id: contract_model.id,
            update_type: ContractUpdateType::ActivateRookie,
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
        transaction_id: ActiveValue::Set(Some(transaction_id)),
        ..Default::default()
    };
    let inserted_team_update =
        team_update_queries::insert_team_update(team_update_to_insert, db).await?;

    Ok(inserted_team_update)
}
