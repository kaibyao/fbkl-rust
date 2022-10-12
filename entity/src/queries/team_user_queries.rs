use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, JoinType, ModelTrait, QueryFilter,
    QuerySelect, RelationTrait, TransactionTrait,
};

use crate::{league, team, team_user, user};

pub async fn get_team_users_by_user<C>(
    user: &user::Model,
    db: &C,
) -> Result<Vec<team_user::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    user.find_related(team_user::Entity).all(db).await
}

pub async fn get_team_user_by_user_and_league<C>(
    user_id: &i64,
    league_id: &i64,
    db: &C,
) -> Result<Option<(team_user::Model, Option<team::Model>)>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    team_user::Entity::find()
        .find_also_related(team::Entity)
        // .join(JoinType::LeftJoin, team_user::Relation::Team.def())
        .join(JoinType::LeftJoin, team::Relation::League.def())
        .filter(team_user::Column::UserId.eq(*user_id))
        .filter(league::Column::Id.eq(*league_id))
        .one(db)
        .await
}
