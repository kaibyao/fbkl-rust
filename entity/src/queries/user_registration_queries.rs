use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, TransactionTrait};

use crate::user_registration;

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
