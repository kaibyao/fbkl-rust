use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter,
    TransactionTrait,
};

use crate::user_registration;

pub async fn insert_user_registration<C>(
    user_registration_to_insert: user_registration::ActiveModel,
    conn: &C,
) -> Result<user_registration::Model, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let inserted_user_registration = user_registration_to_insert.insert(conn).await?;
    Ok(inserted_user_registration)
}

pub async fn find_user_registration_by_token<C>(
    token: Vec<u8>,
    conn: &C,
) -> Result<Option<user_registration::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    user_registration::Entity::find()
        .filter(user_registration::Column::Token.eq(token))
        .one(conn)
        .await
}

pub async fn update_user_registration<C>(
    user_registration_to_update: user_registration::ActiveModel,
    conn: &C,
) -> Result<user_registration::Model, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    let updated_user_registration = user_registration_to_update.update(conn).await?;
    Ok(updated_user_registration)
}
