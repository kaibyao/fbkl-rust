use sea_orm::{ConnectionTrait, DbErr, ModelTrait, TransactionTrait};

use crate::{team, user};

pub async fn find_teams_by_user<C>(user: &user::Model, db: &C) -> Result<Vec<team::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    // team::Entity::find()
    //     .join(JoinType::LeftJoin, team::Relation::TeamUser.def())
    //     .join(JoinType::LeftJoin, team_user::Relation::User.def())
    //     .filter(user::Mode)
    user.find_related(team::Entity).all(db).await
}
