use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

use color_eyre::{eyre::eyre, Result};
use fbkl_entity::{
    contract, contract_queries, deadline_queries, draft_pick, draft_pick_option,
    sea_orm::{
        prelude::DateTimeWithTimeZone, ActiveModelTrait, ActiveValue, ConnectionTrait, LoaderTrait,
    },
    trade::{self, TradeStatus},
    trade_asset::{self, TradeAssetType},
    transaction_queries,
};
use tracing::instrument;

use crate::roster::calculate_team_contract_salary;

use super::{
    create_trade_team_update::{
        generate_team_update_assets_data_for_trade, insert_team_updates_from_completed_trade,
    },
    external_trade_invalidation::invalidate_external_trades_with_traded_assets,
    process_trade_assets, validate_trade_assets,
};

static EMPTY_VEC: &Vec<contract::Model> = &vec![];

/// Stores the trade assets + their related models for a given trade. This exists so that we aren't constantly querying the DB for the same models all the time.
#[derive(Debug)]
pub struct TradeAssetRelatedModelCache {
    pub trade_asset_contracts_by_trade_asset_id:
        HashMap<i64, (trade_asset::Model, contract::Model)>,
    pub trade_asset_draft_picks_by_trade_asset_id:
        HashMap<i64, (trade_asset::Model, draft_pick::Model)>,
    pub trade_asset_draft_pick_options_by_trade_asset_id:
        HashMap<i64, (trade_asset::Model, draft_pick_option::Model)>,
}

impl TradeAssetRelatedModelCache {
    #[instrument]
    pub async fn from_trade_assets<C>(trade_assets: Vec<trade_asset::Model>, db: &C) -> Result<Self>
    where
        C: ConnectionTrait + Debug,
    {
        let mut contract_trade_assets = vec![];
        let mut draft_pick_trade_assets = vec![];
        let mut draft_pick_option_trade_assets = vec![];

        // first group trade assets by their type
        for traded_asset in trade_assets {
            match traded_asset.asset_type {
                TradeAssetType::Contract => contract_trade_assets.push(traded_asset),
                TradeAssetType::DraftPick => draft_pick_trade_assets.push(traded_asset),
                TradeAssetType::DraftPickOption => {
                    draft_pick_option_trade_assets.push(traded_asset)
                }
            };
        }

        let traded_contracts = contract_trade_assets.load_one(contract::Entity, db).await?;
        let trade_asset_contracts_by_trade_asset_id =
            Self::map_trade_asset_models(contract_trade_assets, traded_contracts)?;

        let traded_draft_picks = draft_pick_trade_assets
            .load_one(draft_pick::Entity, db)
            .await?;
        let trade_asset_draft_picks_by_trade_asset_id =
            Self::map_trade_asset_models(draft_pick_trade_assets, traded_draft_picks)?;

        let traded_draft_pick_options = draft_pick_option_trade_assets
            .load_one(draft_pick_option::Entity, db)
            .await?;
        let trade_asset_draft_pick_options_by_trade_asset_id = Self::map_trade_asset_models(
            draft_pick_option_trade_assets,
            traded_draft_pick_options,
        )?;

        let cache = Self {
            trade_asset_contracts_by_trade_asset_id,
            trade_asset_draft_picks_by_trade_asset_id,
            trade_asset_draft_pick_options_by_trade_asset_id,
        };

        Ok(cache)
    }

    fn map_trade_asset_models<M>(
        trade_assets: Vec<trade_asset::Model>,
        related_models: Vec<Option<M>>,
    ) -> Result<HashMap<i64, (trade_asset::Model, M)>> {
        let mut mapped_models = HashMap::new();
        for (trade_asset, maybe_related_model) in
            trade_assets.into_iter().zip(related_models.into_iter())
        {
            let related_model = maybe_related_model.ok_or_else(|| {
                eyre!(
                    "Missing related model for trade asset (id = {}).",
                    trade_asset.id
                )
            })?;
            mapped_models.insert(trade_asset.id, (trade_asset, related_model));
        }

        Ok(mapped_models)
    }
}

/// Moves assets between teams for a created trade, updates the trade status to `completed`, creates the appropriate transaction, and invalidates all other pending trades that include any of the traded assets.
/// Returns the updated trade model.
#[instrument]
pub async fn process_trade<C>(
    trade_model: trade::Model,
    trade_datetime: &DateTimeWithTimeZone,
    db: &C,
) -> Result<trade::Model>
where
    C: ConnectionTrait + Debug,
{
    let next_deadline = deadline_queries::find_next_deadline_for_season_by_datetime(
        trade_model.league_id,
        trade_model.end_of_season_year,
        *trade_datetime,
        db,
    )
    .await?;
    let traded_trade_assets = trade_model.get_trade_assets(db).await?;
    let mut all_team_ids = HashSet::new();
    for traded_trade_asset in traded_trade_assets.iter() {
        all_team_ids.insert(traded_trade_asset.from_team_id);
        all_team_ids.insert(traded_trade_asset.to_team_id);
    }
    let trade_asset_related_models =
        TradeAssetRelatedModelCache::from_trade_assets(traded_trade_assets, db).await?;
    validate_trade_assets(&trade_asset_related_models, trade_model.id, db).await?;

    let active_contracts_by_team_id = contract_queries::find_active_contracts_by_teams(
        all_team_ids.iter().copied().collect(),
        db,
    )
    .await?;
    let mut team_salaries_before_trade = HashMap::new();
    for team_id in all_team_ids.iter() {
        let team_active_contracts = active_contracts_by_team_id
            .get_vec(team_id)
            .unwrap_or(EMPTY_VEC);
        let team_salary_and_cap =
            calculate_team_contract_salary(*team_id, team_active_contracts, &next_deadline, db)
                .await?;
        team_salaries_before_trade.insert(*team_id, team_salary_and_cap);
    }

    // process trade / create new contracts
    let updated_trade_asset_models = process_trade_assets(&trade_asset_related_models, db).await?;
    let updated_trade = update_trade_status(trade_model, db).await?;

    // create transaction
    let trade_transaction =
        transaction_queries::insert_trade_transaction(&next_deadline, updated_trade.id, db).await?;

    // Create team_update
    let trade_asset_contracts: Vec<(trade_asset::Model, contract::Model)> =
        trade_asset_related_models
            .trade_asset_contracts_by_trade_asset_id
            .values()
            .map(|(trade_asset_model, model)| (trade_asset_model.clone(), model.clone()))
            .collect();
    let trade_asset_draft_picks: Vec<(trade_asset::Model, draft_pick::Model)> =
        trade_asset_related_models
            .trade_asset_draft_picks_by_trade_asset_id
            .values()
            .map(|(trade_asset_model, model)| (trade_asset_model.clone(), model.clone()))
            .collect();
    let trade_asset_draft_pick_options: Vec<(trade_asset::Model, draft_pick_option::Model)> =
        trade_asset_related_models
            .trade_asset_draft_pick_options_by_trade_asset_id
            .values()
            .map(|(trade_asset_model, model)| (trade_asset_model.clone(), model.clone()))
            .collect();
    let team_update_assets_by_team_id = generate_team_update_assets_data_for_trade(
        &trade_asset_contracts,
        &trade_asset_draft_picks,
        &trade_asset_draft_pick_options,
        &updated_trade_asset_models.contracts_by_trade_asset_id,
        db,
    )
    .await?;
    insert_team_updates_from_completed_trade(
        team_update_assets_by_team_id,
        trade_datetime,
        &trade_transaction,
        &next_deadline,
        &team_salaries_before_trade,
        all_team_ids.into_iter().collect(),
        db,
    )
    .await?;

    invalidate_external_trades_with_traded_assets(&updated_trade, &trade_asset_related_models, db)
        .await?;

    Ok(updated_trade)
}

#[instrument]
async fn update_trade_status<C>(trade_model: trade::Model, db: &C) -> Result<trade::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut trade_to_update: trade::ActiveModel = trade_model.into();
    trade_to_update.status = ActiveValue::Set(TradeStatus::Completed);
    let updated_trade = trade_to_update.update(db).await?;

    Ok(updated_trade)
}
