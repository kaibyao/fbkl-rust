use crate::{
    models::{user_model::{InsertUser, User}, user_token_model::{InsertUserToken, UserToken}},
    schema::{users, user_tokens},
};
use diesel::{Insertable, RunQueryDsl, r2d2::{PooledConnection, ConnectionManager}, PgConnection, Connection};
use diesel::result::Error;
use diesel::insert_into;
use rand::{rngs::OsRng, RngCore};
use crate::models::user_token_model::TokenTypeEnum;

pub async fn insert(user: InsertUser, conn: &mut PooledConnection<ConnectionManager<PgConnection>>) -> Result<(User, UserToken), Error> {
    conn.transaction::<(User, UserToken), _, _>(|conn| {
        let inserted_user: User = user.insert_into(users::table).get_result(conn)?;

        // generate token
        let mut token = [0u8; 16];
        OsRng.fill_bytes(&mut token);

        let inserted_user_token: UserToken = insert_into(user_tokens::table).values(InsertUserToken {
            user_id: inserted_user.id,
            token: token.into_iter().collect(),
            token_type: TokenTypeEnum::RegistrationConfirm,
            sent_to: Some(inserted_user.email.clone()),
        }).get_result(conn)?;

        Ok((inserted_user, inserted_user_token))
    })
}
