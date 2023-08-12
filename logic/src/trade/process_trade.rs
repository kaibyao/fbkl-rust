use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    contract, contract_queries,
    draft_pick_option::{self, DraftPickOptionStatus},
    sea_orm::{ConnectionTrait, ModelTrait, TransactionTrait},
    trade,
    trade_asset::{self, TradeAssetType},
};
use tracing::instrument;

use super::validate_trade_assets;

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
