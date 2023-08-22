use std::fmt::Debug;

use color_eyre::{eyre::ensure, Result};
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait};
use tracing::instrument;

use crate::trade::{self, TradeStatus};

#[instrument]
pub async fn insert_new_trade<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<trade::Model>
where
    C: ConnectionTrait + Debug,
{
    let trade_model_to_insert = trade::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(end_of_season_year),
        status: ActiveValue::Set(TradeStatus::Proposed),
        league_id: ActiveValue::Set(league_id),
        original_trade_id: ActiveValue::NotSet,
        previous_trade_id: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    let inserted_trade = trade_model_to_insert.insert(db).await?;
    let inserted_trade_id = inserted_trade.id;

    let mut model_to_update: trade::ActiveModel = inserted_trade.into();
    model_to_update.original_trade_id = ActiveValue::Set(Some(inserted_trade_id));
    let updated_trade = model_to_update.update(db).await?;

    Ok(updated_trade)
}

#[instrument]
pub async fn validate_trade_is_latest_in_chain<C>(trade_model: &trade::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let is_latest = trade_model.is_latest_in_chain(db).await?;

    ensure!(
        is_latest,
        "trade_model with id ({}) is not the latest in its chain.",
        trade_model.id
    );

    Ok(())
}
