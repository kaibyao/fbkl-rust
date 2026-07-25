use std::fmt::Debug;

use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, EntityTrait, LoaderTrait, ModelTrait, TransactionTrait,
};
use tracing::instrument;

use crate::{
    contract::{self, ContractStatus},
    contract_queries, draft_pick,
    draft_pick_option::{self, DraftPickOptionStatus},
    trade,
    trade_asset::{self, FromTeamId, ToTeamId, TradeAssetType},
};

#[instrument]
pub async fn get_trade_assets_related_to_contracts<C>(
    contracts: &[contract::Model],
    db: &C,
) -> Result<impl Iterator<Item = trade_asset::Model>>
where
    C: ConnectionTrait + Debug,
{
    let trade_assets_with_contracts = contracts
        .load_many(trade_asset::Entity, db)
        .await?
        .into_iter()
        .flatten();

    Ok(trade_assets_with_contracts)
}

#[instrument]
pub async fn get_trade_assets_related_to_draft_picks<C>(
    draft_picks: Vec<draft_pick::Model>,
    db: &C,
) -> Result<impl Iterator<Item = trade_asset::Model>>
where
    C: ConnectionTrait + Debug,
{
    let trade_assets_with_draft_picks = draft_picks
        .load_many(trade_asset::Entity, db)
        .await?
        .into_iter()
        .flatten();

    Ok(trade_assets_with_draft_picks)
}

#[instrument]
pub async fn get_trade_assets_for_trades<C>(
    trades: &[trade::Model],
    db: &C,
) -> Result<Vec<trade_asset::Model>>
where
    C: ConnectionTrait + Debug,
{
    let trade_assets = trades
        .load_many(trade_asset::Entity, db)
        .await?
        .into_iter()
        .flatten()
        .collect();

    Ok(trade_assets)
}

/// Creates a new, not-yet-inserted trade asset from an asset id, deriving the *sending* team from
/// the asset's current owner in the database so a caller can never trade away another team's asset.
#[instrument]
pub async fn new_trade_asset_active_model_by_id<C>(
    asset_type: TradeAssetType,
    asset_id: i64,
    to_team_id: ToTeamId,
    db: &C,
) -> Result<trade_asset::ActiveModel>
where
    C: ConnectionTrait + Debug,
{
    match asset_type {
        TradeAssetType::Contract => {
            let contract_model = contract_queries::find_contract_by_id(asset_id, db).await?;
            let from_team_id = FromTeamId(
                contract_model
                    .team_id
                    .ok_or_else(|| eyre!("Contract is missing a team_id (id = {asset_id})"))?,
            );
            new_trade_asset_active_model_from_contract(&contract_model, from_team_id, to_team_id)
        }
        TradeAssetType::DraftPick => {
            let draft_pick_model = draft_pick::Entity::find_by_id(asset_id)
                .one(db)
                .await?
                .ok_or_else(|| eyre!("Could not find draft pick (id = {asset_id})"))?;
            Ok(trade_asset::Model::from_draft_pick(
                None,
                draft_pick_model.id,
                FromTeamId(draft_pick_model.current_owner_team_id),
                to_team_id,
            ))
        }
        TradeAssetType::DraftPickOption => {
            let option_model = draft_pick_option::Entity::find_by_id(asset_id)
                .one(db)
                .await?
                .ok_or_else(|| eyre!("Could not find draft pick option (id = {asset_id})"))?;
            if option_model.status != DraftPickOptionStatus::Proposed {
                bail!(
                    "Only a proposed draft pick option can be traded (id = {asset_id}, status = {:?})",
                    option_model.status
                );
            }
            let optioned_draft_pick = option_model
                .find_related(draft_pick::Entity)
                .one(db)
                .await?
                .ok_or_else(|| eyre!("Draft pick option has no draft pick (id = {asset_id})"))?;
            Ok(trade_asset::Model::from_draft_pick_option(
                None,
                option_model.id,
                FromTeamId(optioned_draft_pick.current_owner_team_id),
                to_team_id,
            ))
        }
    }
}

/// Creates a new, not-yet-inserted trade asset from a given contract, without a set `trade_id`.
#[instrument]
pub fn new_trade_asset_active_model_from_contract(
    contract_model: &contract::Model,
    from_team_id: FromTeamId,
    to_team_id: ToTeamId,
) -> Result<trade_asset::ActiveModel> {
    validate_contract_trade_asset(contract_model, from_team_id)?;

    let trade_asset_active_model =
        trade_asset::Model::from_contract(None, contract_model.id, from_team_id, to_team_id);

    Ok(trade_asset_active_model)
}

/// Inserts a new trade (contract) asset for a trade.
#[instrument]
pub async fn insert_trade_asset_from_contract<C>(
    trade_model: &trade::Model,
    contract_model: &contract::Model,
    from_team_id: FromTeamId,
    to_team_id: ToTeamId,
    db: &C,
) -> Result<trade_asset::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let trade_asset_model_to_insert =
        new_trade_asset_active_model_from_contract(contract_model, from_team_id, to_team_id)?;

    let inserted_trade_asset = trade_asset_model_to_insert.insert(db).await?;

    Ok(inserted_trade_asset)
}

fn validate_contract_trade_asset(
    contract_model: &contract::Model,
    from_team_id: FromTeamId,
) -> Result<()> {
    if contract_model.status != ContractStatus::Active {
        bail!(
            "Cannot trade an expired or replaced contract (id = {})",
            contract_model.id
        );
    }

    let contract_team_id = contract_model
        .team_id
        .ok_or_else(|| eyre!("Contract is missing a team_id (id = {})", contract_model.id))?;
    if contract_team_id != from_team_id.0 {
        bail!(
            "Contract's owning team and trade asset's sending team do not match. contract.team_id = {}. trade_asset.from_team_id = {}.",
            contract_team_id,
            from_team_id.0
        );
    }

    Ok(())
}
