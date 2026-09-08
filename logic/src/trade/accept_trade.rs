use std::collections::HashSet;

use color_eyre::{Result, eyre::eyre};
use fbkl_entity::{
    sea_orm::{
        ConnectionTrait, TransactionSession, TransactionTrait, prelude::DateTimeWithTimeZone,
    },
    team_queries, team_user, trade,
    trade_accommodating_drop_queries::{AccommodatingMove, replace_accommodating_drops},
    trade_action::TradeActionType,
    trade_action_queries, trade_queries,
};
use tracing::instrument;

use super::{TradeLegality, process_trade};

/// The proposing team tried to accept its own proposal.
///
/// Concrete (not an opaque `eyre!`) so the resolver can `downcast_ref` and tell the owner why,
/// instead of reporting a bare server fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "team (id = {team_id}) proposed trade (id = {trade_id}), so it cannot accept it: proposing already counts as accepting"
)]
pub struct ProposerCannotAccept {
    pub trade_id: i64,
    pub team_id: i64,
}

/// Accepts a trade by a `team_user`. Also processes the trade if the other teams involved in the trade have already accepted the trade proposal.
///
/// `accommodating_moves` are the contracts the accepting owner drops or sends to the IR to make the
/// trade fit their roster (rules §13.1.5.3). The accept is the owner's one chance to submit them:
/// the moves and the trade's legs are one transaction, judged together when the trade processes
/// (rules §12.5.3, §13.1.4).
///
/// `legality` says who judges the transactions the trade files; an owner-facing accept passes
/// `TradeLegality::JudgeNow`.
///
/// The proposing team cannot accept: `propose_trade` already records its accommodating moves and
/// `has_trade_been_accepted_by_all_teams` already counts its proposal as a response, so a second
/// call from that team would only wipe the moves it submitted.
///
/// Returns an option containing the updated trade if it's been processed, and None otherwise.
#[instrument(skip(db))]
pub async fn accept_trade<C>(
    trade_model: trade::Model,
    accepting_team_user_model: &team_user::Model,
    accept_datetime: &DateTimeWithTimeZone,
    accommodating_moves: &[AccommodatingMove],
    legality: TradeLegality,
    db: &C,
) -> Result<Option<trade::Model>>
where
    C: ConnectionTrait + TransactionTrait,
{
    trade_queries::validate_trade_is_latest_in_chain(&trade_model, db).await?;

    if find_proposing_team_id(&trade_model, db).await? == accepting_team_user_model.team_id {
        return Err(ProposerCannotAccept {
            trade_id: trade_model.id,
            team_id: accepting_team_user_model.team_id,
        }
        .into());
    }

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
        accommodating_moves,
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

/// The team whose owner proposed this trade.
async fn find_proposing_team_id<C>(trade_model: &trade::Model, db: &C) -> Result<i64>
where
    C: ConnectionTrait,
{
    let propose_actions: Vec<_> = trade_model
        .get_trade_actions(db)
        .await?
        .into_iter()
        .filter(|trade_action| trade_action.action_type == TradeActionType::Propose)
        .collect();

    let teams_by_trade_action_ids =
        team_queries::find_teams_by_trade_actions(&propose_actions, db).await?;

    teams_by_trade_action_ids
        .into_values()
        .next()
        .map(|team| team.id)
        .ok_or_else(|| eyre!("trade (id = {}) has no proposal action", trade_model.id))
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
