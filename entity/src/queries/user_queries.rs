use crate::entities::{user, user_registration};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
    TransactionTrait,
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
