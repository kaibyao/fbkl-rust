use color_eyre::eyre::Result;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};

use crate::user_registration;

pub async fn find_user_registration_by_token<C>(
    token: Vec<u8>,
    conn: &C,
) -> Result<Option<user_registration::Model>>
where
    C: ConnectionTrait + TransactionTrait,
{
    Ok(user_registration::Entity::find()
        .filter(user_registration::Column::Token.eq(token))
        .one(conn)
        .await?)
}
