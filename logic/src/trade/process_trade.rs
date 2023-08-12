use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionTrait},
    trade::{self, TradeStatus},
};
use tracing::instrument;

use super::{process_trade_assets, validate_trade_assets};

/// Moves assets between teams for a created trade, updates the trade status to completed, and creates the appropriate transaction.
#[instrument]
pub async fn process_trade<C>(trade_model: trade::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let traded_trade_assets = trade_model.get_traded_assets(db).await?;
    validate_trade_assets(&traded_trade_assets, &trade_model, db).await?;

    let db_transaction = db.begin().await?;

    process_trade_assets(&traded_trade_assets, &db_transaction).await?;
    update_trade_status(trade_model, &db_transaction).await?;

    // create transaction
    // invalidate other trades that may involve any of the moved trade assets

    db_transaction.commit().await?;

    Ok(())
}

async fn update_trade_status<C>(trade_model: trade::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let mut trade_to_update: trade::ActiveModel = trade_model.into();
    trade_to_update.status = ActiveValue::Set(TradeStatus::Completed);
    trade_to_update.update(db).await?;

    Ok(())
}
