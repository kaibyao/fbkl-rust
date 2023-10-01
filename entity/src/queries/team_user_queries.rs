use std::fmt::Debug;

use color_eyre::eyre::{eyre, Result};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, JoinType, ModelTrait, QueryFilter, QuerySelect,
    RelationTrait,
};
use tracing::instrument;

use crate::{league, team, team_user, user};

/// Retrieves the default team user for a team in a given season
#[instrument]
pub async fn find_default_team_user_for_team<C>(
    team_model: &team::Model,
    end_of_season_year: i16,
    db: &C,
) -> Result<team_user::Model>
where
    C: ConnectionTrait + Debug,
{
    let team_user_models = team_model.get_team_users(db).await?;
    let default_team_user = team_user_models
        .into_iter()
        .find(|team_user_model| {
            team_user_model.first_end_of_season_year <= end_of_season_year
                && team_user_model
                    .final_end_of_season_year
                    .map_or(true, |year| year >= end_of_season_year)
        })
        .ok_or_else(|| eyre!("Could not find a default team_user for team (team_id = {}) and end-of-season year {}", team_model.id, end_of_season_year))?;
    Ok(default_team_user)
}

#[instrument]
pub async fn get_team_users_by_team<C>(team_id: i64, db: &C) -> Result<Vec<team_user::Model>>
where
    C: ConnectionTrait + Debug,
{
    let team_users = team_user::Entity::find()
        .join(JoinType::LeftJoin, team_user::Relation::Team.def())
        .filter(team_user::Column::TeamId.eq(team_id))
        .all(db)
        .await?;

    Ok(team_users)
}

#[instrument]
pub async fn get_team_users_by_user<C>(user: &user::Model, db: &C) -> Result<Vec<team_user::Model>>
where
    C: ConnectionTrait + Debug,
{
    let team_users = user.find_related(team_user::Entity).all(db).await?;
    Ok(team_users)
}

#[instrument]
pub async fn get_team_user_by_user_and_league<C>(
    user_id: &i64,
    league_id: &i64,
    db: &C,
) -> Result<Option<(team_user::Model, Option<team::Model>)>>
where
    C: ConnectionTrait + Debug,
{
    let team_users = team_user::Entity::find()
        .find_also_related(team::Entity)
        .join(JoinType::LeftJoin, team::Relation::League.def())
        .filter(team_user::Column::UserId.eq(*user_id))
        .filter(league::Column::Id.eq(*league_id))
        .one(db)
        .await?;
    Ok(team_users)
}
