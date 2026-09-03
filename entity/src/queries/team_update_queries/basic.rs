use color_eyre::Result;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, JoinType, ModelTrait,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, sea_query::Expr,
};
use tracing::instrument;

use crate::{
    deadline, league_event,
    team_update::{self, TeamUpdateStatus},
};

/// Finds the `team_updates` related to the given deadline.
#[instrument(skip(db))]
pub async fn find_team_updates_for_deadline<C>(
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait,
{
    let team_updates = deadline_model
        .find_related(team_update::Entity)
        .all(db)
        .await?;
    Ok(team_updates)
}

/// Finds the `team_updates` related to the given league event id.
#[instrument(skip(db))]
pub async fn find_team_updates_by_league_event<C>(
    league_event_id: i64,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait,
{
    let team_updates = team_update::Entity::find()
        .filter(team_update::Column::LeagueEventId.eq(league_event_id))
        .all(db)
        .await?;
    Ok(team_updates)
}

/// Finds a team's `team_updates` newest-first, narrowed by status and/or deadline.
///
/// Filtering by deadline gives the moves made in one week, since every roster move records its
/// league event against the deadline it is made for.
#[instrument(skip(db))]
pub async fn find_team_updates_by_team<C>(
    team_id: i64,
    maybe_status: Option<TeamUpdateStatus>,
    maybe_deadline_id: Option<i64>,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait,
{
    let mut query = team_update::Entity::find().filter(team_update::Column::TeamId.eq(team_id));

    if let Some(status) = maybe_status {
        query = query.filter(team_update::Column::Status.eq(status));
    }

    if let Some(deadline_id) = maybe_deadline_id {
        query = query
            .join(
                JoinType::InnerJoin,
                team_update::Relation::LeagueEvent.def(),
            )
            .filter(league_event::Column::DeadlineId.eq(deadline_id));
    }

    let team_updates = query.order_by_desc(team_update::Column::Id).all(db).await?;
    Ok(team_updates)
}

#[instrument(skip(db))]
pub async fn insert_team_update<C>(
    team_update_to_insert: team_update::ActiveModel,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait,
{
    let inserted_team_update = team_update_to_insert.insert(db).await?;
    Ok(inserted_team_update)
}

#[instrument(skip(db))]
pub async fn update_team_update_status<C>(
    team_update_model: team_update::Model,
    status: TeamUpdateStatus,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait,
{
    let mut setting_status_to_in_progress: team_update::ActiveModel = team_update_model.into();
    setting_status_to_in_progress.status = ActiveValue::Set(status);
    let status_set_to_in_progress = setting_status_to_in_progress.update(db).await?;
    Ok(status_set_to_in_progress)
}

/// A team's moves for one week that were written after `after_team_update_id`.
///
/// Ids ascend, so the newest id read before a batch of moves is applied marks off the rows that
/// batch wrote: the rows of one transaction (rules §13.1.4). The logic fns each write their own
/// `team_update` and return only the contract, so this is how a caller collects what it just did.
#[instrument(skip(db))]
pub async fn find_team_updates_after<C>(
    team_id: i64,
    deadline_id: i64,
    after_team_update_id: i64,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait,
{
    let mut week_moves = find_team_updates_by_team(team_id, None, Some(deadline_id), db).await?;
    week_moves.retain(|team_update| team_update.id > after_team_update_id);
    Ok(week_moves)
}

/// Where a new transaction starts in a team's week (rules §13.1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionStart {
    /// The newest move already on record, which marks off the rows the new transaction writes.
    pub after_team_update_id: i64,
    /// The number the new transaction takes: one past the highest the week already stores, so a
    /// submission never lands in a transaction already judged.
    pub transaction_number: i16,
}

/// Reads where a team's next transaction of the week starts, before its moves are applied.
///
/// Unnumbered rows are each their own transaction and hold no number to avoid, so only the stored
/// numbers decide the next one.
#[instrument(skip(db))]
pub async fn find_transaction_start<C>(
    team_id: i64,
    deadline_id: i64,
    db: &C,
) -> Result<TransactionStart>
where
    C: ConnectionTrait,
{
    let week_moves = find_team_updates_by_team(team_id, None, Some(deadline_id), db).await?;
    let highest_stored = week_moves
        .iter()
        .filter_map(|team_update| team_update.transaction_number)
        .max();

    Ok(TransactionStart {
        after_team_update_id: week_moves.first().map_or(0, |team_update| team_update.id),
        transaction_number: highest_stored.map_or(0, |number| number.saturating_add(1)),
    })
}

/// Puts every one of `team_update_ids` in one transaction, so its moves are judged together
/// (rules §13.1.4).
#[instrument(skip(db))]
pub async fn assign_team_updates_to_transaction<C>(
    transaction_number: i16,
    team_update_ids: &[i64],
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    if team_update_ids.is_empty() {
        return Ok(());
    }

    team_update::Entity::update_many()
        .col_expr(
            team_update::Column::TransactionNumber,
            Expr::value(transaction_number),
        )
        .filter(team_update::Column::Id.is_in(team_update_ids.iter().copied()))
        .exec(db)
        .await?;

    Ok(())
}

/// Writes the owner's chosen transaction order over one week's `team_updates` (rules §13.1.1).
///
/// Each inner list is one transaction: its position in `ordered_transactions` becomes the
/// transaction number stored on every move in it, so those moves are judged together (§13.1.4).
#[instrument(skip(db))]
pub async fn update_team_update_transaction_numbers<C>(
    ordered_transactions: &[Vec<i64>],
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    for (index, team_update_ids) in ordered_transactions.iter().enumerate() {
        assign_team_updates_to_transaction(i16::try_from(index)?, team_update_ids, db).await?;
    }

    Ok(())
}

#[instrument(skip(db))]
pub async fn update_team_updates_with_status<C>(
    team_update_model_ids: Vec<i64>,
    status: TeamUpdateStatus,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait,
{
    let updated_models = team_update::Entity::update_many()
        .col_expr(team_update::Column::Status, Expr::value(status))
        .filter(team_update::Column::Id.is_in(team_update_model_ids))
        .exec_with_returning(db)
        .await?;

    Ok(updated_models)
}
