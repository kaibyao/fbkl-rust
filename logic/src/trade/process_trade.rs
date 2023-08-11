use std::fmt::Debug;

use color_eyre::{
    eyre::{ensure, eyre},
    Result,
};
use fbkl_entity::{
    contract, contract_queries,
    draft_pick_option::{self, DraftPickOptionStatus},
    sea_orm::{ConnectionTrait, ModelTrait, TransactionTrait},
    trade,
    trade_asset::{self, TradeAssetType},
};
use tracing::instrument;

/// Moves assets between teams for a created trade, updates the trade status to completed, and creates the appropriate transaction.
#[instrument]
pub async fn process_trade<C>(trade_model: &trade::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let traded_trade_assets = trade_model.get_traded_assets(db).await?;

    validate_trade_assets(&traded_trade_assets, trade_model, db).await?;

    // move + update referenced trade assets
    // update trade status
    // create transaction
    // invalidate other trades that may involve any of the moved trade assets

    Ok(())
}

#[instrument]
async fn validate_trade_assets<C>(
    trade_asset_models: &[trade_asset::Model],
    trade_model: &trade::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    for trade_asset_model in trade_asset_models {
        match trade_asset_model.asset_type {
            TradeAssetType::Contract => {
                validate_contract_trade_asset(trade_asset_model, trade_model, db).await?;
            }
            TradeAssetType::DraftPick => {
                validate_draft_pick_trade_asset(trade_asset_model, trade_model, db).await?;
            }
            TradeAssetType::DraftPickOption => {
                validate_draft_pick_option_trade_asset(trade_asset_model, db).await?;
            }
            TradeAssetType::DraftPickOptionAmendment => {
                validate_draft_pick_option_amendment_trade_asset(trade_asset_model, db).await?;
            }
        }
    }

    Ok(())
}

#[instrument]
async fn validate_contract_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    trade_model: &trade::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_model = trade_asset_model.get_contract(db).await?;

    // Ensure it's still the latest contract.
    contract_queries::validate_contract_is_latest_in_chain(&contract_model, db).await?;

    // Ensure contract is owned by the team referenced by the trade asset entity
    ensure!(contract_model.team_id.map_or(false, |team_id| team_id == trade_asset_model.from_team_id), "Contract ({}) is not currently owned by the team listed as the owning team in this trade ({}). Contract's owning team = {:?}. Trade's listed owning team = {}.", contract_model.id, trade_model.id, contract_model.team_id, trade_asset_model.from_team_id);

    Ok(contract_model)
}

#[instrument]
async fn validate_draft_pick_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    trade_model: &trade::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let draft_pick_model = trade_asset_model.get_draft_pick(db).await?;

    // Ensure draft pick is owned by the team referenced in the trade asset entity
    ensure!(draft_pick_model.current_owner_team_id == trade_asset_model.from_team_id, "Draft pick ({}) is not currently owned by the team listed as the owning team in this trade ({}). Draft pick's owning team = {:?}. Trade's listed owning team = {}.", draft_pick_model.id, trade_model.id, draft_pick_model.current_owner_team_id, trade_asset_model.from_team_id);

    Ok(())
}

#[instrument]
async fn validate_draft_pick_option_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let draft_pick_option = trade_asset_model.get_draft_pick_option(db).await?;

    // Ensure that the draft pick option is in a proposed status, as they can't be traded after creation.
    ensure!(
        draft_pick_option.status == DraftPickOptionStatus::Proposed,
        "Draft pick option (id={}) must have a `Proposed` status (owners can only trade new draft pick options). (status = {:?})",
        draft_pick_option.id,
        draft_pick_option.status
    );

    Ok(())
}

#[instrument]
async fn validate_draft_pick_option_amendment_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let draft_pick_option_amendment = trade_asset_model
        .get_draft_pick_option_amendment(db)
        .await?;
    let draft_pick_option = draft_pick_option_amendment
        .find_related(draft_pick_option::Entity)
        .one(db)
        .await?
        .ok_or_else(|| {
            eyre!(
                "Could not find draft pick option related to draft pick option amendment (id = {})",
                draft_pick_option_amendment.id
            )
        })?;

    // Ensure that the draft pick amendment's targeted draft pick option is in a valid status, as they can't be traded after creation.
    ensure!(
        draft_pick_option.status != DraftPickOptionStatus::Active,
        "Draft pick option must be active for an amendment to be processed."
    );

    Ok(())
}
