use std::fmt::Debug;

use color_eyre::{
    eyre::{ensure, eyre},
    Result,
};
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, ModelTrait, TransactionTrait};
use tracing::instrument;

use crate::{
    team, team_trade, team_user,
    trade::{self, TradeStatus},
    trade_action::TradeActionType,
    trade_action_queries, trade_asset,
};

/// Creates & inserts a new trade proposed by a team to 1 or more teams. Inserts the following entities: The (proposed) trade, the team_trades involved, the trade assets involved, and the proposal trade action
/// Trades have to be created w/ this method in order to set the `original_trade_id` after insertion.
#[instrument]
pub async fn propose_new_trade<C>(
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

    // insert team_trade records
    let from_team_model = proposing_team_user_model
        .find_related(team::Entity)
        .one(db)
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
            trade_id: ActiveValue::Set(updated_trade.id),
        };
        let _inserted_team_trade_model = team_trade_to_insert.insert(db).await?;
    }

    // insert trade_asset records
    for mut trade_asset_to_insert in proposed_trade_assets {
        trade_asset_to_insert.trade_id = ActiveValue::Set(updated_trade.id);
        let _inserted_trade_asset = trade_asset_to_insert.insert(db).await?;
    }

    // create trade action for proposal
    let _proposed_trade_action = trade_action_queries::insert_trade_action(
        TradeActionType::Propose,
        updated_trade.id,
        proposing_team_user_model.id,
        db,
    )
    .await?;

    Ok(updated_trade)
}

pub async fn validate_trade_is_latest_in_chain<C>(trade_model: &trade::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let is_latest = trade_model.is_latest_in_chain(db).await?;

    ensure!(
        is_latest,
        "trade_model with id ({}) is not the latest in its chain.",
        trade_model.id
    );

    Ok(())
}
