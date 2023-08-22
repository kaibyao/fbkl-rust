use std::fmt::Debug;

use color_eyre::{eyre::eyre, Result};
use fbkl_entity::{
    sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, ModelTrait, TransactionTrait},
    team, team_trade, team_user, trade,
    trade_action::TradeActionType,
    trade_action_queries, trade_asset, trade_queries,
};
use tracing::instrument;

/// Creates & inserts a new trade proposed by a team to 1 or more teams. Inserts the following entities: The (proposed) trade, the team_trades involved, the trade assets involved, and the proposal trade action
/// Trades have to be created w/ this method in order to set the `original_trade_id` after insertion.
#[instrument]
pub async fn propose_trade<C>(
    league_id: i64,
    end_of_season_year: i16,
    proposing_team_user_model: &team_user::Model,
    to_team_ids: &[i64],
    proposed_trade_assets: Vec<trade_asset::ActiveModel>,
    db: &C,
) -> Result<trade::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let db_txn = db.begin().await?;

    let inserted_trade =
        trade_queries::insert_new_trade(league_id, end_of_season_year, &db_txn).await?;

    // insert team_trade records
    let from_team_model = proposing_team_user_model
        .find_related(team::Entity)
        .one(&db_txn)
        .await?
        .ok_or_else(|| {
            eyre!(
                "Could not fetch team model related to team_user: {} ({})",
                proposing_team_user_model.nickname,
                proposing_team_user_model.id
            )
        })?;

    let mut all_team_ids = vec![];
    all_team_ids.push(from_team_model.id);
    all_team_ids.extend(to_team_ids);
    for team_id in all_team_ids {
        let team_trade_to_insert = team_trade::ActiveModel {
            id: ActiveValue::NotSet,
            team_id: ActiveValue::Set(team_id),
            trade_id: ActiveValue::Set(inserted_trade.id),
        };
        let _inserted_team_trade_model = team_trade_to_insert.insert(&db_txn).await?;
    }

    // insert trade_asset records
    for mut trade_asset_to_insert in proposed_trade_assets {
        trade_asset_to_insert.trade_id = ActiveValue::Set(inserted_trade.id);
        let _inserted_trade_asset = trade_asset_to_insert.insert(&db_txn).await?;
    }

    // create trade action for proposal
    let _proposed_trade_action = trade_action_queries::insert_trade_action(
        TradeActionType::Propose,
        inserted_trade.id,
        proposing_team_user_model.id,
        &db_txn,
    )
    .await?;

    db_txn.commit().await?;

    Ok(inserted_trade)
}
