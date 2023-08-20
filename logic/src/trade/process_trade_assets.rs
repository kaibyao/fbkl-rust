use std::{collections::HashMap, fmt::Debug};

use color_eyre::Result;
use fbkl_entity::{
    contract::{self, ContractStatus},
    draft_pick,
    draft_pick_option::{self, DraftPickOptionStatus},
    sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait},
};
use tracing::instrument;

use super::process_trade::TradeAssetRelatedModelCache;

/// Stores the updated, related models of the trade's trade asset models.
#[derive(Debug)]
pub struct UpdatedTradeAssetModelCache {
    pub contracts_by_trade_asset_id: HashMap<i64, contract::Model>,
    pub draft_picks_by_trade_asset_id: HashMap<i64, draft_pick::Model>,
    pub draft_pick_options_by_trade_asset_id: HashMap<i64, draft_pick_option::Model>,
}

#[instrument]
pub async fn process_trade_assets<C>(
    trade_asset_related_models: &TradeAssetRelatedModelCache,
    db: &C,
) -> Result<UpdatedTradeAssetModelCache>
where
    C: ConnectionTrait + Debug,
{
    let mut updated_contracts: HashMap<i64, contract::Model> = HashMap::new();
    for (trade_asset_id, (trade_asset_model, contract_model)) in trade_asset_related_models
        .trade_asset_contracts_by_trade_asset_id
        .iter()
    {
        let updated_contract =
            update_trade_asset_contract(contract_model, trade_asset_model.to_team_id, db).await?;
        updated_contracts.insert(*trade_asset_id, updated_contract);
    }

    let mut updated_draft_picks: HashMap<i64, draft_pick::Model> = HashMap::new();
    for (trade_asset_id, (trade_asset_model, draft_pick_model)) in trade_asset_related_models
        .trade_asset_draft_picks_by_trade_asset_id
        .iter()
    {
        let updated_draft_pick =
            update_trade_asset_draft_pick(draft_pick_model, trade_asset_model.to_team_id, db)
                .await?;
        updated_draft_picks.insert(*trade_asset_id, updated_draft_pick);
    }

    let mut updated_draft_pick_options: HashMap<i64, draft_pick_option::Model> = HashMap::new();
    for (trade_asset_id, (_trade_asset_model, draft_pick_option_model)) in
        trade_asset_related_models
            .trade_asset_draft_pick_options_by_trade_asset_id
            .iter()
    {
        let updated_draft_pick_option =
            update_trade_asset_draft_pick_option(draft_pick_option_model, db).await?;
        updated_draft_pick_options.insert(*trade_asset_id, updated_draft_pick_option);
    }

    Ok(UpdatedTradeAssetModelCache {
        contracts_by_trade_asset_id: updated_contracts,
        draft_picks_by_trade_asset_id: updated_draft_picks,
        draft_pick_options_by_trade_asset_id: updated_draft_pick_options,
    })
}

#[instrument]
async fn update_trade_asset_contract<C>(
    contract_model: &contract::Model,
    new_team_id: i64,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut new_contract: contract::ActiveModel = contract_model.clone().into();

    new_contract.team_id = ActiveValue::Set(Some(new_team_id));
    new_contract.previous_contract_id = ActiveValue::Set(Some(contract_model.id));
    let mut existing_contract_to_update: contract::ActiveModel = contract_model.clone().into();
    existing_contract_to_update.status = ActiveValue::Set(ContractStatus::Replaced);

    existing_contract_to_update.update(db).await?;
    let updated_contract = new_contract.update(db).await?;

    Ok(updated_contract)
}

#[instrument]
async fn update_trade_asset_draft_pick<C>(
    draft_pick_model: &draft_pick::Model,
    new_team_id: i64,
    db: &C,
) -> Result<draft_pick::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut draft_pick_to_update: draft_pick::ActiveModel = draft_pick_model.clone().into();
    draft_pick_to_update.current_owner_team_id = ActiveValue::Set(new_team_id);
    let updated_draft_pick = draft_pick_to_update.update(db).await?;

    Ok(updated_draft_pick)
}

#[instrument]
async fn update_trade_asset_draft_pick_option<C>(
    draft_pick_option_model: &draft_pick_option::Model,
    db: &C,
) -> Result<draft_pick_option::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut draft_pick_option_to_update: draft_pick_option::ActiveModel =
        draft_pick_option_model.clone().into();

    draft_pick_option_to_update.status = ActiveValue::Set(DraftPickOptionStatus::Active);
    let updated_draft_pick_option = draft_pick_option_to_update.update(db).await?;

    Ok(updated_draft_pick_option)
}
