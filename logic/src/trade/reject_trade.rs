use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionTrait},
    team_user,
    trade::{self, TradeStatus},
    trade_action::TradeActionType,
    trade_action_queries, trade_queries,
};
use tracing::instrument;

/// Rejects a proposed trade: records the rejecting `team_user`'s `Reject` action and closes the trade.
///
/// Draft-pick options carried by the trade are left as-is; cancelling them
/// (`DraftPickOptionStatus::CancelledViaTradeRejection`) belongs with the draft work in spec 02.
#[instrument]
pub async fn reject_trade<C>(
    trade_model: trade::Model,
    rejecting_team_user_model: &team_user::Model,
    db: &C,
) -> Result<trade::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    trade_queries::validate_trade_is_latest_in_chain(&trade_model, db).await?;

    let db_txn = db.begin().await?;

    let _rejected_trade_action = trade_action_queries::insert_trade_action(
        TradeActionType::Reject,
        trade_model.id,
        rejecting_team_user_model.id,
        &db_txn,
    )
    .await?;

    let mut trade_to_update: trade::ActiveModel = trade_model.into();
    trade_to_update.status = ActiveValue::Set(TradeStatus::Rejected);
    let updated_trade = trade_to_update.update(&db_txn).await?;

    db_txn.commit().await?;

    Ok(updated_trade)
}
