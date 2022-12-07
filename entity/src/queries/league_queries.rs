use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, DbErr, ModelTrait, QueryFilter,
    TransactionTrait,
};

use crate::{
    league, team,
    team_user::{self, LeagueRole},
    user,
};

pub async fn find_league_by_user<C>(
    user: &user::Model,
    league_id: i64,
    db: &C,
) -> Result<Option<league::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    user.find_linked(team_user::Entity)
        .filter(league::Column::Id.eq(league_id))
        .one(db)
        .await
}

pub async fn find_leagues_by_user<C>(
    user: &user::Model,
    db: &C,
) -> Result<Vec<league::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    user.find_linked(team_user::Entity).all(db).await
}

pub async fn create_league<C>(
    league_name: String,
    team_name: String,
    league_owner_user_id: i64,
    user_nickname: String,
    db: &C,
) -> Result<(league::Model, team::Model, team_user::Model), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let transaction = db.begin().await?;

    let inserted_league = league::ActiveModel {
        name: ActiveValue::Set(league_name),
        ..Default::default()
    }
    .insert(&transaction)
    .await?;

    let inserted_team = team::ActiveModel {
        name: ActiveValue::Set(team_name),
        league_id: ActiveValue::Set(inserted_league.id),
        ..Default::default()
    }
    .insert(&transaction)
    .await?;

    let inserted_team_user = team_user::ActiveModel {
        league_role: ActiveValue::Set(LeagueRole::LeagueCommissioner),
        nickname: ActiveValue::Set(user_nickname),
        team_id: ActiveValue::Set(inserted_team.id),
        user_id: ActiveValue::Set(league_owner_user_id),
        ..Default::default()
    }
    .insert(&transaction)
    .await?;

    transaction.commit().await?;

    Ok((inserted_league, inserted_team, inserted_team_user))
}
