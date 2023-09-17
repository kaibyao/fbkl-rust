use color_eyre::{eyre::eyre, Result};
use fbkl_constants::FREE_AGENCY_TEAM;
use fbkl_entity::{
    deadline,
    sea_orm::{prelude::DateTimeWithTimeZone, ActiveValue, ConnectionTrait, EntityTrait},
};
use multimap::MultiMap;
use std::{collections::HashMap, fmt::Debug};
use tracing::instrument;

use fbkl_entity::{
    contract::{self, RelatedPlayer},
    contract_queries, draft_pick, draft_pick_option, draft_pick_queries,
    team_update::{
        self, ContractUpdate, ContractUpdateType, DraftPickUpdate, DraftPickUpdateType,
        TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    trade_asset, transaction,
};

use crate::roster::calculate_team_contract_salary;

static EMPTY_VEC: &Vec<contract::Model> = &vec![];

/// Generates the data needed to create the team updates related to a trade. Returns a MultiMap w/ `team_id`s as its key and Team Update Assets as its values.
#[instrument]
pub async fn generate_team_update_assets_data_for_trade<C>(
    trade_asset_contracts: &[(trade_asset::Model, contract::Model)],
    trade_asset_draft_picks: &[(trade_asset::Model, draft_pick::Model)],
    trade_asset_draft_pick_options: &[(trade_asset::Model, draft_pick_option::Model)],
    updated_contracts_by_trade_asset_id: &HashMap<i64, contract::Model>,
    db: &C,
) -> Result<MultiMap<i64, TeamUpdateAsset>>
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

    Ok(team_update_assets_by_team_id)
}

/// Creates & inserts a team update from a completed trade.
#[instrument]
pub async fn insert_team_updates_from_completed_trade<C>(
    team_update_assets_by_team_id: MultiMap<i64, TeamUpdateAsset>,
    trade_datetime: &DateTimeWithTimeZone,
    trade_transaction: &transaction::Model,
    deadline_model: &deadline::Model,
    team_salaries_before_trade: &HashMap<i64, (i16, i16)>,
    team_ids_involved_in_trade: Vec<i64>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    // get all contracts + draft picks per team; the new contracts have already been created by this point
    let active_contracts_by_team_id =
        contract_queries::find_active_contracts_by_teams(team_ids_involved_in_trade, db).await?;

    // Insert new team updates
    let mut team_update_models_to_insert = vec![];
    for (team_id, team_update_assets) in team_update_assets_by_team_id.into_iter() {
        let team_active_contracts = active_contracts_by_team_id
            .get_vec(&team_id)
            .unwrap_or(EMPTY_VEC);
        let team_contract_ids = team_active_contracts
            .iter()
            .map(|contract_model| contract_model.id)
            .collect();
        let (new_salary, new_salary_cap) =
            calculate_team_contract_salary(team_id, team_active_contracts, deadline_model, db)
                .await?;
        let (previous_salary, previous_salary_cap) =
            team_salaries_before_trade.get(&team_id).expect(
                "Team salaries should have been generated using all team_ids involved in trade.",
            );

        let team_update_data = TeamUpdateData::from_assets(
            team_contract_ids,
            team_update_assets,
            new_salary,
            new_salary_cap,
            *previous_salary,
            *previous_salary_cap,
        );
        let new_team_update = team_update::ActiveModel {
            id: ActiveValue::NotSet,
            data: ActiveValue::Set(team_update_data.to_json()?),
            effective_date: ActiveValue::Set(trade_datetime.date_naive()),
            status: ActiveValue::Set(TeamUpdateStatus::Done),
            team_id: ActiveValue::Set(team_id),
            transaction_id: ActiveValue::Set(Some(trade_transaction.id)),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        };
        team_update_models_to_insert.push(new_team_update);
    }

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
                player_name_at_time: player_name.clone(),
                player_team_abbr_at_time: team_abbr.clone(),
                player_team_name_at_time: team_name.clone(),
            },
        );

        // traded for update uses latest contract
        team_update_contract_assets_by_team_id.insert(
            trade_asset_model.to_team_id,
            ContractUpdate {
                contract_id: updated_contract_model.id,
                update_type: ContractUpdateType::AddViaTrade,
                player_name_at_time: player_name,
                player_team_abbr_at_time: team_abbr,
                player_team_name_at_time: team_name,
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
