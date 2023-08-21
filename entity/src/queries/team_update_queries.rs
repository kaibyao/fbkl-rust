use chrono::{NaiveDate, Utc};
use color_eyre::{eyre::eyre, Result};
use fbkl_constants::FREE_AGENCY_TEAM;
use multimap::MultiMap;
use sea_orm::{
    prelude::DateTimeWithTimeZone, ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait,
    EntityTrait, QueryFilter,
};
use std::{collections::HashMap, fmt::Debug};
use tracing::instrument;

use crate::{
    auction_bid,
    contract::{self, ContractType, RelatedPlayer},
    contract_queries, draft_pick, draft_pick_option, draft_pick_queries,
    prelude::TeamUpdate,
    team,
    team_update::{
        self, ContractUpdate, ContractUpdateType, DraftPickUpdate, DraftPickUpdateType,
        TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    trade_asset, transaction,
};

/// Finds the team_updates related to the given transaction id.
#[instrument]
pub async fn find_team_updates_by_transaction<C>(
    transaction_id: i64,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait + Debug,
{
    let team_updates = team_update::Entity::find()
        .filter(team_update::Column::TransactionId.eq(transaction_id))
        .all(db)
        .await?;
    Ok(team_updates)
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

/// Creates & inserts a team update from a completed auction.
pub async fn insert_team_update_from_auction_won<C>(
    winning_auction_bid_model: &auction_bid::Model,
    auction_transaction_model: &transaction::Model,
    signed_contract_model: &contract::Model,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_update_player_data =
        ContractUpdatePlayerData::from_contract_model(signed_contract_model, db).await?;

    let data = TeamUpdateData::Assets(vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
        contract_id: signed_contract_model.id,
        player_name_at_time_of_trade: contract_update_player_data.player_name,
        player_team_abbr_at_time_of_trade: contract_update_player_data.real_team_abbr,
        player_team_name_at_time_of_trade: contract_update_player_data.real_team_name,
        update_type: ContractUpdateType::AddViaAuction,
    }])]);
    let deadline_model = auction_transaction_model.get_deadline(db).await?;
    let team_model = winning_auction_bid_model.get_team(db).await?;
    let new_team_update = team_update::ActiveModel {
        id: ActiveValue::NotSet,
        data: ActiveValue::Set(data.as_bytes()?),
        effective_date: ActiveValue::Set(deadline_model.date_time.date_naive()),
        status: ActiveValue::Set(TeamUpdateStatus::Pending),
        team_id: ActiveValue::Set(team_model.id),
        transaction_id: ActiveValue::Set(Some(auction_transaction_model.id)),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    let inserted_team_update = new_team_update.insert(db).await?;

    Ok(inserted_team_update)
}

/// Creates & inserts a team update from a completed trade.
pub async fn insert_team_updates_from_completed_trade<C>(
    trade_asset_contracts: &[(trade_asset::Model, contract::Model)],
    trade_asset_draft_picks: &[(trade_asset::Model, draft_pick::Model)],
    trade_asset_draft_pick_options: &[(trade_asset::Model, draft_pick_option::Model)],
    updated_contracts_by_trade_asset_id: &HashMap<i64, contract::Model>,
    trade_datetime: &DateTimeWithTimeZone,
    trade_transaction: &transaction::Model,
    db: &C,
) -> Result</*team_update::Model*/ ()>
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

    TeamUpdate::insert_many(team_update_models_to_insert)
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

#[instrument]
pub async fn update_team_update_status<C>(
    team_update_model: team_update::Model,
    status: TeamUpdateStatus,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut setting_status_to_in_progress: team_update::ActiveModel = team_update_model.into();
    setting_status_to_in_progress.status = ActiveValue::Set(status);
    let status_set_to_in_progress = setting_status_to_in_progress.update(db).await?;
    Ok(status_set_to_in_progress)
}

/// Updates the given team_update (generated via veteran auction processing) to be finished, along with an optional effective date (defaults to `now()` otherwise).
#[instrument]
pub async fn update_team_update_for_preseason_veteran_auction<C>(
    team_update_model: &team_update::Model,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut update_team_update_date_and_status: team_update::ActiveModel =
        team_update_model.clone().into();
    update_team_update_date_and_status.status = ActiveValue::Set(TeamUpdateStatus::Done);
    update_team_update_date_and_status.effective_date =
        ActiveValue::Set(maybe_override_effective_date.unwrap_or_else(|| Utc::now().date_naive()));

    let updated_model = update_team_update_date_and_status.update(db).await?;
    Ok(updated_model)
}

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

struct ContractUpdatePlayerData {
    player_name: String,
    real_team_abbr: String,
    real_team_name: String,
}

impl ContractUpdatePlayerData {
    #[instrument]
    pub async fn from_contract_model<C>(contract_model: &contract::Model, db: &C) -> Result<Self>
    where
        C: ConnectionTrait + Debug,
    {
        let player_model = contract_model.get_player(db).await?;

        let data = match player_model {
            RelatedPlayer::LeaguePlayer(league_player_model) => Self {
                player_name: league_player_model.name,
                real_team_abbr: FREE_AGENCY_TEAM.2.to_string(),
                real_team_name: format!("{} {}", FREE_AGENCY_TEAM.0, FREE_AGENCY_TEAM.1),
            },
            RelatedPlayer::Player(player_model) => {
                let real_team_model = player_model.get_real_team(db).await?;
                Self {
                    player_name: player_model.name,
                    real_team_abbr: real_team_model.code,
                    real_team_name: format!("{} {}", &real_team_model.city, &real_team_model.name),
                }
            }
        };

        Ok(data)
    }
}
