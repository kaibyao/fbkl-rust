use std::collections::HashSet;

use color_eyre::eyre::{Result, eyre};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, JoinType, LoaderTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, TransactionSession, TransactionTrait,
    prelude::DateTimeWithTimeZone,
};
use tracing::instrument;

use crate::{
    draft_pick, draft_pick_draft_pick_option, draft_pick_option,
    trade::{self, TradeStatus},
    trade_action::{self, TradeActionType},
    trade_asset::{self, TradeAssetType},
};

#[instrument(skip(db))]
pub async fn insert_draft_pick<C>(
    draft_pick_model: draft_pick::ActiveModel,
    db: &C,
) -> Result<draft_pick::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let inserted_draft_pick_model = draft_pick_model.insert(db).await?;
    Ok(inserted_draft_pick_model)
}

#[instrument(skip(db))]
pub async fn insert_draft_picks<C>(draft_picks: Vec<draft_pick::ActiveModel>, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait,
{
    let transaction = db.begin().await?;
    draft_pick::Entity::insert_many(draft_picks)
        .exec(&transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

#[instrument(skip(db))]
pub async fn get_draft_picks_affected_by_options<C>(
    draft_pick_options: &[draft_pick_option::Model],
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let related_draft_picks: Vec<draft_pick::Model> = draft_pick_options
        .load_many_to_many(draft_pick::Entity, draft_pick_draft_pick_option::Entity, db)
        .await?
        .into_iter()
        .flatten()
        .collect();

    Ok(related_draft_picks)
}

#[instrument(skip(db))]
pub async fn find_draft_pick_by_id<C>(draft_pick_id: i64, db: &C) -> Result<draft_pick::Model>
where
    C: ConnectionTrait,
{
    draft_pick::Entity::find_by_id(draft_pick_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find draft pick ({draft_pick_id})."))
}

#[instrument(skip(db))]
pub async fn get_draft_picks_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let draft_picks = draft_pick::Entity::find()
        .filter(
            draft_pick::Column::LeagueId
                .eq(league_id)
                .and(draft_pick::Column::EndOfSeasonYear.eq(end_of_season_year)),
        )
        .order_by_asc(draft_pick::Column::Round)
        .order_by_asc(draft_pick::Column::Id)
        .all(db)
        .await?;

    Ok(draft_picks)
}

/// Ids of the draft picks `team_id` received in a trade announced after `announced_after`.
///
/// Rules §15.2.2 bars a team from forfeiting a pick it picked up after the moment it is measured
/// against, so the RFA compensation set subtracts this. Pick-transfer rules need the same subtraction.
///
/// Announcement time is the accepting `trade_action`'s `created_at`: a trade is processed the moment
/// its last team accepts, and that row is never rewritten afterwards, unlike `trade.updated_at`
/// (which moves again when the trade's transaction id is written back).
#[instrument(skip(db))]
pub async fn find_draft_pick_ids_acquired_by_team_after<C>(
    team_id: i64,
    announced_after: DateTimeWithTimeZone,
    db: &C,
) -> Result<HashSet<i64>>
where
    C: ConnectionTrait,
{
    let acquisitions = trade_asset::Entity::find()
        .filter(trade_asset::Column::ToTeamId.eq(team_id))
        .filter(trade_asset::Column::AssetType.eq(TradeAssetType::DraftPick))
        .join(JoinType::InnerJoin, trade_asset::Relation::Trade.def())
        .filter(trade::Column::Status.eq(TradeStatus::Completed))
        .join(JoinType::InnerJoin, trade::Relation::TradeAction.def())
        .filter(trade_action::Column::ActionType.eq(TradeActionType::Accept))
        .filter(trade_action::Column::CreatedAt.gt(announced_after))
        .all(db)
        .await?;

    Ok(acquisitions
        .into_iter()
        .filter_map(|acquisition| acquisition.draft_pick_id)
        .collect())
}
