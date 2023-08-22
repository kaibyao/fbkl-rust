use std::{collections::HashSet, fmt::Debug};

use color_eyre::Result;
use fbkl_entity::{
    sea_orm::{prelude::DateTimeWithTimeZone, ConnectionTrait, TransactionTrait},
    team_queries, team_user, trade,
    trade_action::TradeActionType,
    trade_action_queries, trade_queries,
};
use tracing::instrument;

use super::process_trade;

/// Accepts a trade by a team_user. Also processes the trade if the other teams involved in the trade have already accepted the trade proposal.
/// Returns an option containing the updated trade if it's been processed, and None otherwise.
#[instrument]
pub async fn accept_trade<C>(
    trade_model: trade::Model,
    accepting_team_user_model: &team_user::Model,
    accept_datetime: &DateTimeWithTimeZone,
    db: &C,
) -> Result<Option<trade::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    trade_queries::validate_trade_is_latest_in_chain(&trade_model, db).await?;

    let db_txn = db.begin().await?;

    let _accepted_trade_action = trade_action_queries::insert_trade_action(
        TradeActionType::Accept,
        trade_model.id,
        accepting_team_user_model.id,
        &db_txn,
    )
    .await?;

    // check if other teams have already accepted and if so, process the trade.
    let maybe_processed_trade =
        if has_trade_been_accepted_by_all_teams(&trade_model, &db_txn).await? {
            Some(process_trade(trade_model, accept_datetime, &db_txn).await?)
        } else {
            None
        };

    db_txn.commit().await?;

    Ok(maybe_processed_trade)
}

async fn has_trade_been_accepted_by_all_teams<C>(trade_model: &trade::Model, db: &C) -> Result<bool>
where
    C: ConnectionTrait + Debug,
{
    let all_trade_actions = trade_model.get_trade_actions(db).await?;
    for trade_action in all_trade_actions.iter() {
        // if there are teams for which their latest trade action was not one of [accept, propose], then the trade has not been accepted by all parties.
        match trade_action.action_type {
            TradeActionType::Propose | TradeActionType::Accept => continue,
            _ => return Ok(false),
        }
    }

    let teams_by_trade_action_ids =
        team_queries::find_teams_by_trade_actions(&all_trade_actions, db).await?;
    let all_trade_teams = trade_model.get_teams(db).await?;

    // if there are teams for which trade actions are missing, then not every team has accepted the trade.
    let ids_of_teams_that_responded: HashSet<i64> = teams_by_trade_action_ids
        .values()
        .map(|team| team.id)
        .collect();
    let all_trade_team_ids: HashSet<i64> = all_trade_teams.iter().map(|team| team.id).collect();
    let did_all_teams_respond = all_trade_team_ids
        .difference(&ids_of_teams_that_responded)
        .collect::<Vec<&i64>>()
        .is_empty();
    if !did_all_teams_respond {
        return Ok(false);
    }

    Ok(true)
}
