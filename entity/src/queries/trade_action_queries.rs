use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ConnectionTrait, TransactionTrait};
use tracing::instrument;

use crate::trade_action::{self, TradeActionType};

/// Inserts a new trade (contract) asset for a trade.
#[instrument]
pub async fn insert_trade_action<C>(
    trade_action_type: TradeActionType,
    trade_id: i64,
    team_user_id: i64,
    db: &C,
) -> Result<trade_action::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let trade_action_to_insert =
        trade_action::Model::new_active_model(trade_action_type, trade_id, team_user_id);
    let inserted_trade_action = trade_action_to_insert.insert(db).await?;

    Ok(inserted_trade_action)
}
