use std::fmt::Debug;

use color_eyre::eyre::Result;
use fbkl_entity::{
    contract, deadline,
    sea_orm::{ActiveValue, ConnectionTrait, EntityTrait},
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries::ContractUpdatePlayerData,
};
use multimap::MultiMap;
use tracing::instrument;

#[instrument]
pub async fn create_team_updates_for_advanced_team_contracts<C>(
    advanced_team_contracts: &[contract::Model],
    preseason_start_deadline_model: &deadline::Model,
    transaction_id: i64,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let contracts_by_team: MultiMap<i64, &contract::Model> = advanced_team_contracts
        .iter()
        .map(|contract_model| {
            (
                contract_model
                    .team_id
                    .expect("A contract that was advanced should belong to a team."),
                contract_model,
            )
        })
        .collect();

    // team update data
    let mut team_updates_to_insert = vec![];
    for (team_id, team_contracts) in contracts_by_team.iter_all() {
        let mut team_contract_ids = vec![];
        let mut contract_updates = vec![];

        for contract_model in team_contracts {
            team_contract_ids.push(contract_model.id);

            let contract_update_player_data =
                ContractUpdatePlayerData::from_contract_model(contract_model, db).await?;
            contract_updates.push(ContractUpdate {
                contract_id: contract_model.id,
                update_type: ContractUpdateType::ContractAdvanced,
                player_name_at_time_of_trade: contract_update_player_data.player_name,
                player_team_abbr_at_time_of_trade: contract_update_player_data.real_team_abbr,
                player_team_name_at_time_of_trade: contract_update_player_data.real_team_name,
            });
        }

        let team_update_data = TeamUpdateData::from_assets(
            team_contract_ids,
            vec![TeamUpdateAsset::Contracts(contract_updates)],
            0,
            0,
            0,
            0,
        );
        team_updates_to_insert.push(team_update::ActiveModel {
            data: ActiveValue::Set(team_update_data.as_bytes()?),
            effective_date: ActiveValue::Set(preseason_start_deadline_model.date_time.date_naive()),
            status: ActiveValue::Set(TeamUpdateStatus::Done),
            team_id: ActiveValue::Set(*team_id),
            transaction_id: ActiveValue::Set(Some(transaction_id)),
            ..Default::default()
        });
    }

    team_update::Entity::insert_many(team_updates_to_insert)
        .exec(db)
        .await?;

    Ok(())
}
