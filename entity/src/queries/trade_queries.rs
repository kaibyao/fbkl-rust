use std::fmt::Debug;

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, JoinType,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use tracing::instrument;

use crate::{
    team_trade,
    trade::{self, TradeStatus},
};

/// Statuses a team can still act on (accept / reject / counter).
const ACTIVE_TRADE_STATUSES: [TradeStatus; 2] =
    [TradeStatus::Proposed, TradeStatus::Counteroffered];

#[instrument]
pub async fn find_trade_by_id<C>(trade_id: i64, db: &C) -> Result<trade::Model>
where
    C: ConnectionTrait + Debug,
{
    trade::Entity::find_by_id(trade_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find trade (id = {})", trade_id))
}

/// Every still-actionable trade in a league, newest first.
#[instrument]
pub async fn find_active_trades_in_league<C>(league_id: i64, db: &C) -> Result<Vec<trade::Model>>
where
    C: ConnectionTrait + Debug,
{
    let trades = trade::Entity::find()
        .filter(trade::Column::LeagueId.eq(league_id))
        .filter(trade::Column::Status.is_in(ACTIVE_TRADE_STATUSES))
        .order_by_desc(trade::Column::Id)
        .all(db)
        .await?;

    Ok(trades)
}

/// Every still-actionable trade a team is involved in (as proposer or recipient), newest first.
#[instrument]
pub async fn find_active_trades_for_team<C>(team_id: i64, db: &C) -> Result<Vec<trade::Model>>
where
    C: ConnectionTrait + Debug,
{
    let trades = trade::Entity::find()
        .join(JoinType::InnerJoin, team_trade::Relation::Trade.def().rev())
        .filter(team_trade::Column::TeamId.eq(team_id))
        .filter(trade::Column::Status.is_in(ACTIVE_TRADE_STATUSES))
        .order_by_desc(trade::Column::Id)
        .distinct()
        .all(db)
        .await?;

    Ok(trades)
}

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
        transaction_id: ActiveValue::NotSet,
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
