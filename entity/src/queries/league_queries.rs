use sea_orm::{ConnectionTrait, DbErr, ModelTrait, TransactionTrait};

use crate::{league, team_user, user};

pub async fn find_leagues_by_user<C>(
    user: &user::Model,
    db: &C,
) -> Result<Vec<league::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    user.find_linked(team_user::Entity).all(db).await
}
