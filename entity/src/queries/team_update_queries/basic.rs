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

/// Writes the owner's chosen order over one week's `team_updates` (rules §13.1.1).
///
/// The position in `ordered_team_update_ids` becomes the stored sequence. Ordering is
/// presentational and for the audit log, so no roster validator reads what this writes.
#[instrument(skip(db))]
pub async fn update_team_update_sequences<C>(ordered_team_update_ids: &[i64], db: &C) -> Result<()>
where
    C: ConnectionTrait,
{
    for (index, team_update_id) in ordered_team_update_ids.iter().enumerate() {
        let sequence = i16::try_from(index)?;
        team_update::Entity::update_many()
            .col_expr(team_update::Column::Sequence, Expr::value(sequence))
            .filter(team_update::Column::Id.eq(*team_update_id))
            .exec(db)
            .await?;
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
