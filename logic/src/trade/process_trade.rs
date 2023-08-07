use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    contract, contract_queries,
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
    // let involved_team_models = trade_model.get_teams(db).await?;
    let traded_asset_models = trade_model.get_traded_assets(db).await?;

    for trade_asset_model in traded_asset_models {
        match trade_asset_model.asset_type {
            TradeAssetType::Contract => {
                validate_contract_trade_asset(&trade_asset_model, db).await?;
            }
            TradeAssetType::DraftPick => {
                validate_draft_pick_trade_asset(&trade_asset_model, db).await?;
            }
            TradeAssetType::DraftPickOption => {
                validate_draft_pick_option_trade_asset(&trade_asset_model, db).await?;
            }
        }
    }

    Ok(())
}

#[instrument]
async fn validate_contract_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_model = trade_asset_model.get_contract(db).await?;

    // Need to ensure it's still the latest contract.
    contract_queries::validate_contract_is_latest_in_chain(&contract_model, db).await?;

    Ok(contract_model)
}

#[instrument]
async fn validate_draft_pick_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
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
    // Need to ensure that the draft pick this option is attached to is either being traded to the new team or belongs on the new team already.
    Ok(())
}
