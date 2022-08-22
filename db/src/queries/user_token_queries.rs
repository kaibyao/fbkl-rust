use crate::{
    models::user_token_model::{TokenTypeEnum, UserToken},
    schema::user_tokens::{dsl::user_tokens, token, token_type},
};
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, PooledConnection},
    result::Error,
    PgConnection,
};

pub fn find_by_token_and_type(
    token_param: Vec<u8>,
    token_type_param: TokenTypeEnum,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<UserToken, Error> {
    user_tokens
        .filter(token_type.eq(token_type_param))
        .filter(token.eq(token_param))
        .first::<UserToken>(conn)
}
