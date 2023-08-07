use crate::{
    entities::{user, user_registration},
    league, team, team_user,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, JoinType, QueryFilter,
    QuerySelect, RelationTrait, Set, TransactionTrait,
};

/// Inserts a new user + registration. Requires a token that's used for registration confirmation.
pub async fn insert_user<C>(
    user_to_insert: user::ActiveModel,
    token: Vec<u8>,
    conn: &C,
) -> Result<(user::Model, user_registration::Model), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let transaction = conn.begin().await?;

    let inserted_user = user_to_insert.insert(&transaction).await?;
    let inserted_user_registration = user_registration::ActiveModel {
        user_id: Set(inserted_user.id),
        token: Set(token),
        ..Default::default()
    }
    .insert(&transaction)
    .await?;

    transaction.commit().await?;
    Ok((inserted_user, inserted_user_registration))
}

pub async fn find_user_by_email<C>(email: &str, conn: &C) -> Result<Option<user::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    user::Entity::find()
        .filter(user::Column::Email.eq(email))
        .one(conn)
        .await
}

pub async fn find_users_by_league_id<C>(league_id: i64, conn: &C) -> Result<Vec<user::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    user::Entity::find()
        .join(JoinType::LeftJoin, user::Relation::TeamUser.def())
        .join(JoinType::LeftJoin, team_user::Relation::Team.def())
        .join(JoinType::LeftJoin, team::Relation::League.def())
        .filter(league::Column::Id.eq(league_id))
        .all(conn)
        .await
}
