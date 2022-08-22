use crate::{
    models::{
        user_model::{InsertUser, UpdateUser, User},
        user_token_model::{InsertUserToken, TokenTypeEnum, UserToken},
    },
    schema::{user_tokens, users},
};
use diesel::{
    insert_into,
    r2d2::{ConnectionManager, PooledConnection},
    result::Error,
    Connection, Insertable, PgConnection, RunQueryDsl, SaveChangesDsl,
};

/// Inserts a new user. Requires a token that's used for registration confirmation.
pub fn insert_user(
    user: InsertUser,
    token: Vec<u8>,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<(User, UserToken), Error> {
    conn.transaction::<(User, UserToken), _, _>(|conn| {
        let inserted_user: User = user.insert_into(users::table).get_result(conn)?;

        let inserted_user_token: UserToken = insert_into(user_tokens::table)
            .values(InsertUserToken {
                user_id: inserted_user.id,
                token,
                token_type: TokenTypeEnum::RegistrationConfirm,
                sent_to: Some(inserted_user.email.clone()),
            })
            .get_result(conn)?;

        Ok((inserted_user, inserted_user_token))
    })
}

pub fn update_user(
    user: UpdateUser,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<User, Error> {
    user.save_changes(conn)
}
