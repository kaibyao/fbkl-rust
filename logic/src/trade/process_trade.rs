use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    deadline_queries,
    sea_orm::{prelude::DateTimeWithTimeZone, ActiveModelTrait, ActiveValue, ConnectionTrait},
    trade::{self, TradeStatus},
    transaction_queries,
};
use tracing::instrument;

use super::{
    external_trade_invalidation::invalidate_external_trades_with_traded_assets,
    process_trade_assets, validate_trade_assets,
};

/// Moves assets between teams for a created trade, updates the trade status to `completed`, creates the appropriate transaction, and invalidates all other pending trades that include any of the traded assets.
#[instrument]
pub async fn process_trade<C>(
    trade_model: trade::Model,
    trade_datetime: &DateTimeWithTimeZone,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let traded_trade_assets = trade_model.get_trade_assets(db).await?;
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
    transaction_queries::insert_trade_transaction(&next_deadline, updated_trade.id, db).await?;

    // Create team_update
    todo!();

    invalidate_external_trades_with_traded_assets(&updated_trade, traded_trade_assets, db).await
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
