use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    sea_orm::{ConnectionTrait, TransactionTrait},
    trade,
    trade_asset::TradeAssetType,
};
use tracing::instrument;

/// Moves assets between teams for a trade, and creates the appropriate transaction.
#[instrument]
pub async fn process_trade<C>(trade_model: &trade::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let involved_team_models = trade_model.get_teams(db).await?;
    let traded_asset_models = trade_model.get_traded_assets(db).await?;

    for asset_model in traded_asset_models {
        match asset_model.asset_type {
            TradeAssetType::Contract => todo!(),
            TradeAssetType::DraftPick => todo!(),
            TradeAssetType::DraftPickOption => todo!(),
        }
    }

    Ok(())
}
