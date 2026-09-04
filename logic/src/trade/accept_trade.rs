use std::collections::HashSet;

use color_eyre::Result;
use fbkl_entity::{
    sea_orm::{
        ConnectionTrait, TransactionSession, TransactionTrait, prelude::DateTimeWithTimeZone,
    },
    team_queries, team_user, trade,
    trade_accommodating_drop_queries::replace_accommodating_drops,
    trade_action::TradeActionType,
    trade_action_queries, trade_queries,
};
use tracing::instrument;

use super::{TradeLegality, process_trade};

/// Accepts a trade by a `team_user`. Also processes the trade if the other teams involved in the trade have already accepted the trade proposal.
///
/// `accommodating_drop_contract_ids` are the contracts the accepting owner drops to make the trade
/// fit their roster. The accept is the owner's one chance to submit them: the drops and the trade's
/// legs are one transaction, judged together when the trade processes (rules §12.5.3, §13.1.4).
///
/// `legality` says who judges the transactions the trade files; an owner-facing accept passes
/// `TradeLegality::JudgeNow`.
///
/// Returns an option containing the updated trade if it's been processed, and None otherwise.
#[instrument(skip(db))]
pub async fn accept_trade<C>(
    trade_model: trade::Model,
    accepting_team_user_model: &team_user::Model,
    accept_datetime: &DateTimeWithTimeZone,
    accommodating_drop_contract_ids: &[i64],
    legality: TradeLegality,
    db: &C,
) -> Result<Option<trade::Model>>
where
    C: ConnectionTrait + TransactionTrait,
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

    replace_accommodating_drops(
        trade_model.id,
        accepting_team_user_model.team_id,
        accommodating_drop_contract_ids,
        &db_txn,
    )
    .await?;

    // check if other teams have already accepted and if so, process the trade.
    let maybe_processed_trade =
        if has_trade_been_accepted_by_all_teams(&trade_model, &db_txn).await? {
            Some(process_trade(trade_model, accept_datetime, legality, &db_txn).await?)
        } else {
            None
        };

    db_txn.commit().await?;

    Ok(maybe_processed_trade)
}

async fn has_trade_been_accepted_by_all_teams<C>(trade_model: &trade::Model, db: &C) -> Result<bool>
where
    C: ConnectionTrait,
{
    let all_trade_actions = trade_model.get_trade_actions(db).await?;
    let all_actions_are_accept_or_propose = all_trade_actions.iter().all(|trade_action| {
        matches!(
            trade_action.action_type,
            TradeActionType::Propose | TradeActionType::Accept
        )
    });
    if !all_actions_are_accept_or_propose {
        return Ok(false);
    }

    let teams_by_trade_action_ids =
        team_queries::find_teams_by_trade_actions(&all_trade_actions, db).await?;
    let all_trade_teams = trade_model.get_teams(db).await?;

    let ids_of_teams_that_responded: HashSet<i64> = teams_by_trade_action_ids
        .values()
        .map(|team| team.id)
        .collect();
    let all_trade_team_ids: HashSet<i64> = all_trade_teams.iter().map(|team| team.id).collect();

    Ok(all_trade_team_ids.is_subset(&ids_of_teams_that_responded))
}
