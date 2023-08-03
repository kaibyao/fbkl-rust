use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    sea_orm::{ConnectionTrait, TransactionTrait},
    team_user, trade,
    trade_action::TradeActionType,
    trade_action_queries, trade_queries,
};
use tracing::instrument;

use super::process_trade;

/// Accepts a trade by a team_user. Also processes the trade if the other teams involved in the trade have already accepted the trade proposal.
#[instrument]
pub async fn accept_trade<C>(
    trade_model: &trade::Model,
    accepting_team_user_model: &team_user::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    trade_queries::validate_trade_is_latest_in_chain(trade_model, db).await?;

    let db_txn = db.begin().await?;

    let _accepted_trade_action = trade_action_queries::insert_trade_action(
        TradeActionType::Accept,
        trade_model.id,
        accepting_team_user_model.id,
        db,
    )
    .await?;

    // check if other teams have already accepted and if so, process the trade.
    if has_trade_been_accepted_by_all_teams(trade_model, db).await? {
        process_trade(trade_model, db).await?;
    }

    db_txn.commit().await?;

    Ok(())
}

#[instrument]
async fn has_trade_been_accepted_by_all_teams<C>(trade_model: &trade::Model, db: &C) -> Result<bool>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    Ok(true)
}
