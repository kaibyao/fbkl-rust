use crate::{
    models::user_model::{InsertUser, User},
    schema::users,
};
use diesel::{Insertable, QueryResult, RunQueryDsl, r2d2::{PooledConnection, ConnectionManager}, PgConnection};

pub async fn insert(user: InsertUser, conn: &mut PooledConnection<ConnectionManager<PgConnection>>) -> QueryResult<User> {
    user.insert_into(users::table).get_result(conn)
}
