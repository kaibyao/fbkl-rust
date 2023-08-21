use color_eyre::{eyre::eyre, Result};
use fbkl_constants::FREE_AGENCY_TEAM;
use multimap::MultiMap;
use sea_orm::{prelude::DateTimeWithTimeZone, ActiveValue, ConnectionTrait, EntityTrait};
use std::{collections::HashMap, fmt::Debug};
use tracing::instrument;

use crate::{
    contract::{self, RelatedPlayer},
    draft_pick, draft_pick_option, draft_pick_queries,
    team_update::{
        self, ContractUpdate, ContractUpdateType, DraftPickUpdate, DraftPickUpdateType,
        TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    trade_asset, transaction,
};

/// Creates & inserts a team update from a completed trade.
pub async fn insert_team_updates_from_completed_trade<C>(
    trade_asset_contracts: &[(trade_asset::Model, contract::Model)],
    trade_asset_draft_picks: &[(trade_asset::Model, draft_pick::Model)],
    trade_asset_draft_pick_options: &[(trade_asset::Model, draft_pick_option::Model)],
    updated_contracts_by_trade_asset_id: &HashMap<i64, contract::Model>,
    trade_datetime: &DateTimeWithTimeZone,
    trade_transaction: &transaction::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    // Iterate through traded assets and append team asset updates to each team's update data.
    let team_update_contract_assets_by_team_id = get_team_update_contract_updates(
        trade_asset_contracts,
        updated_contracts_by_trade_asset_id,
        db,
    )
    .await?;
    let team_update_draft_pick_assets_by_team_id = get_team_update_draft_pick_updates(
        trade_asset_draft_picks,
        trade_asset_draft_pick_options,
        db,
    )
    .await?;

    // Create TeamUpdateAssets
    let mut team_update_assets_by_team_id: MultiMap<i64, TeamUpdateAsset> = MultiMap::new();
    for (team_id, contract_updates) in team_update_contract_assets_by_team_id.into_iter() {
        team_update_assets_by_team_id.insert(team_id, TeamUpdateAsset::Contracts(contract_updates));
    }
    for (team_id, draft_pick_updates) in team_update_draft_pick_assets_by_team_id.into_iter() {
        team_update_assets_by_team_id
            .insert(team_id, TeamUpdateAsset::DraftPicks(draft_pick_updates));
    }

    // Insert new team updates
    let team_update_models_to_insert: Vec<team_update::ActiveModel> = team_update_assets_by_team_id
        .into_iter()
        .map(|(team_id, team_update_asset)| {
            let team_update_data = TeamUpdateData::Assets(team_update_asset);
            let new_team_update = team_update::ActiveModel {
                id: ActiveValue::NotSet,
                data: ActiveValue::Set(team_update_data.as_bytes()?),
                effective_date: ActiveValue::Set(trade_datetime.date_naive()),
                status: ActiveValue::Set(TeamUpdateStatus::Done),
                team_id: ActiveValue::Set(team_id),
                transaction_id: ActiveValue::Set(Some(trade_transaction.id)),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            };
            Ok(new_team_update)
        })
        .collect::<Result<Vec<team_update::ActiveModel>>>()?;

    team_update::Entity::insert_many(team_update_models_to_insert)
        .exec(db)
        .await?;

    Ok(())
}

#[instrument]
async fn get_team_update_contract_updates<C>(
    trade_asset_contracts: &[(trade_asset::Model, contract::Model)],
    updated_contracts_by_trade_asset_id: &HashMap<i64, contract::Model>,
    db: &C,
) -> Result<MultiMap<i64, ContractUpdate>>
where
    C: ConnectionTrait + Debug,
{
    let mut team_update_contract_assets_by_team_id: MultiMap<i64, ContractUpdate> = MultiMap::new();

    for (trade_asset_model, contract_model) in trade_asset_contracts {
        let updated_contract_model = updated_contracts_by_trade_asset_id.get(&trade_asset_model.id).ok_or_else(|| eyre!("Could not get updated contract for trade asset (id = {}) and traded contract (id = {})", trade_asset_model.id, contract_model.id))?;

        let (player_name, team_abbr, team_name) = match contract_model.get_player(db).await? {
            RelatedPlayer::LeaguePlayer(league_player_model) => (
                league_player_model.name,
                FREE_AGENCY_TEAM.2.to_string(),
                format!("{} {}", FREE_AGENCY_TEAM.0, FREE_AGENCY_TEAM.1),
            ),
            RelatedPlayer::Player(player_model) => {
                let real_team_model = player_model.get_real_team(db).await?;
                (
                    player_model.name,
                    real_team_model.code,
                    real_team_model.name,
                )
            }
        };

        // traded away update uses current contract (now replaced)
        team_update_contract_assets_by_team_id.insert(
            trade_asset_model.from_team_id,
            ContractUpdate {
                contract_id: contract_model.id,
                update_type: ContractUpdateType::TradedAway,
                player_name_at_time_of_trade: player_name.clone(),
                player_team_abbr_at_time_of_trade: team_abbr.clone(),
                player_team_name_at_time_of_trade: team_name.clone(),
            },
        );

        // traded for update uses latest contract
        team_update_contract_assets_by_team_id.insert(
            trade_asset_model.to_team_id,
            ContractUpdate {
                contract_id: updated_contract_model.id,
                update_type: ContractUpdateType::AddViaTrade,
                player_name_at_time_of_trade: player_name,
                player_team_abbr_at_time_of_trade: team_abbr,
                player_team_name_at_time_of_trade: team_name,
            },
        );
    }

    Ok(team_update_contract_assets_by_team_id)
}

#[instrument]
async fn get_team_update_draft_pick_updates<C>(
    trade_asset_draft_picks: &[(trade_asset::Model, draft_pick::Model)],
    trade_asset_draft_pick_options: &[(trade_asset::Model, draft_pick_option::Model)],
    db: &C,
) -> Result<MultiMap<i64, DraftPickUpdate>>
where
    C: ConnectionTrait + Debug,
{
    let mut team_update_draft_pick_assets_by_team_id: MultiMap<i64, DraftPickUpdate> =
        MultiMap::new();

    for (trade_asset_model, draft_pick_model) in trade_asset_draft_picks {
        team_update_draft_pick_assets_by_team_id.insert(
            trade_asset_model.from_team_id,
            DraftPickUpdate {
                draft_pick_id: draft_pick_model.id,
                update_type: DraftPickUpdateType::TradedAway,
                added_draft_pick_option_id: None,
            },
        );
        team_update_draft_pick_assets_by_team_id.insert(
            trade_asset_model.to_team_id,
            DraftPickUpdate {
                draft_pick_id: draft_pick_model.id,
                update_type: DraftPickUpdateType::AddViaTrade,
                added_draft_pick_option_id: None,
            },
        );
    }

    let draft_pick_options: Vec<draft_pick_option::Model> = trade_asset_draft_pick_options
        .iter()
        .map(|(_, draft_pick_option_model)| draft_pick_option_model.clone())
        .collect();
    let draft_picks_affected_by_options =
        draft_pick_queries::get_draft_picks_affected_by_options(&draft_pick_options, db).await?;
    for ((trade_asset_model, draft_pick_option_model), draft_pick_model) in
        trade_asset_draft_pick_options
            .iter()
            .zip(draft_picks_affected_by_options.iter())
    {
        team_update_draft_pick_assets_by_team_id.insert(
            trade_asset_model.from_team_id,
            DraftPickUpdate {
                draft_pick_id: draft_pick_model.id,
                update_type: DraftPickUpdateType::DraftPickOptionAdded,
                added_draft_pick_option_id: Some(draft_pick_option_model.id),
            },
        );
    }

    Ok(team_update_draft_pick_assets_by_team_id)
}
