use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    sea_orm::{ConnectionTrait, TransactionTrait},
    trade,
    trade_asset::{self, TradeAssetType},
};
use tracing::instrument;

/// Moves assets between teams for a created trade, and creates the appropriate transaction.
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
) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    // let trade_asset_contract_model = trade_asset_model.find_related(_)

    // Need to ensure it's still the latest contract.
    Ok(())
}

#[instrument]
async fn validate_draft_pick_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    Ok(())
}

#[instrument]
async fn validate_draft_pick_option_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    // Need to ensure that the draft pick this option is attached to is either being traded to the new team or belongs on the new team already.
    Ok(())
}
