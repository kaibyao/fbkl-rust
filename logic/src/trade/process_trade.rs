use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    deadline_queries,
    sea_orm::{prelude::DateTimeWithTimeZone, ActiveModelTrait, ActiveValue, ConnectionTrait},
    trade::{self, TradeStatus},
    trade_asset, transaction,
};
use tracing::instrument;

use super::{process_trade_assets, validate_trade_assets};

/// Moves assets between teams for a created trade, updates the trade status to completed, and creates the appropriate transaction.
#[instrument]
pub async fn process_trade<C>(
    trade_model: trade::Model,
    trade_datetime: &DateTimeWithTimeZone,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let traded_trade_assets = trade_model.get_traded_assets(db).await?;
    validate_trade_assets(&traded_trade_assets, &trade_model, db).await?;

    process_trade_assets(&traded_trade_assets, db).await?;
    let updated_trade = update_trade_status(trade_model, db).await?;

    // create transaction
    let next_deadline = deadline_queries::find_next_deadline_for_season_by_datetime(
        updated_trade.league_id,
        updated_trade.end_of_season_year,
        *trade_datetime,
        db,
    )
    .await?;
    let transaction_to_insert =
        transaction::Model::new_trade_transaction(&next_deadline, &updated_trade);
    let _inserted_transaction = transaction_to_insert.insert(db).await?;

    // invalidate other trades that may involve any of the moved trade assets
    invalidate_external_trades_with_traded_assets(&traded_trade_assets, db).await?;

    Ok(())
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

#[instrument]
async fn invalidate_external_trades_with_traded_assets<C>(
    traded_assets: &[trade_asset::Model],
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    Ok(())
}
