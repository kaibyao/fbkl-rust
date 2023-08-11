use std::fmt::Debug;

use color_eyre::{eyre::ensure, Result};
use fbkl_entity::{
    contract, contract_queries,
    draft_pick_option::DraftPickOptionStatus,
    sea_orm::{ConnectionTrait, TransactionTrait},
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

    // Ensure that the draft pick option has a valid status.
    ensure!(
        draft_pick_option.status == DraftPickOptionStatus::Proposed
            || draft_pick_option.status == DraftPickOptionStatus::Active,
        "Draft pick option (id={}) must have a valid status. (status = {:?})",
        draft_pick_option.id,
        draft_pick_option.status
    );

    // Getting sleepy thinking about how someone may trade a draft pick option on its own without trading the attached pick, or how a pick may be traded without its option(s)

    // The naiive

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
    Ok(())
}
