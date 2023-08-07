use std::fmt::Debug;

use color_eyre::{eyre::ensure, Result};
use sea_orm::ConnectionTrait;
use tracing::instrument;

use crate::trade;

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
