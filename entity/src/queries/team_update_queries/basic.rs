use color_eyre::Result;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait,
    ModelTrait, QueryFilter,
};
use std::fmt::Debug;
use tracing::instrument;

use crate::{
    deadline,
    team_update::{self, TeamUpdateStatus},
};

/// Finds the team_updates related to the given deadline.
#[instrument]
pub async fn find_team_updates_for_deadline<C>(
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait + Debug,
{
    let team_updates = deadline_model
        .find_related(team_update::Entity)
        .all(db)
        .await?;
    Ok(team_updates)
}

/// Finds the team_updates related to the given transaction id.
#[instrument]
pub async fn find_team_updates_by_transaction<C>(
    transaction_id: i64,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait + Debug,
{
    let team_updates = team_update::Entity::find()
        .filter(team_update::Column::TransactionId.eq(transaction_id))
        .all(db)
        .await?;
    Ok(team_updates)
}

#[instrument]
pub async fn insert_team_update<C>(
    team_update_to_insert: team_update::ActiveModel,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let inserted_team_update = team_update_to_insert.insert(db).await?;
    Ok(inserted_team_update)
}

#[instrument]
pub async fn update_team_update_status<C>(
    team_update_model: team_update::Model,
    status: TeamUpdateStatus,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut setting_status_to_in_progress: team_update::ActiveModel = team_update_model.into();
    setting_status_to_in_progress.status = ActiveValue::Set(status);
    let status_set_to_in_progress = setting_status_to_in_progress.update(db).await?;
    Ok(status_set_to_in_progress)
}

#[instrument]
pub async fn update_team_updates_with_status<C>(
    team_update_model_ids: Vec<i64>,
    status: TeamUpdateStatus,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait + Debug,
{
    let updated_models = team_update::Entity::update_many()
        .col_expr(team_update::Column::Status, Expr::value(status))
        .filter(team_update::Column::Id.is_in(team_update_model_ids))
        .exec_with_returning(db)
        .await?;

    Ok(updated_models)
}
