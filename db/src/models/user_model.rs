use chrono::{DateTime, Utc};
use diesel::{Identifiable, Queryable, Insertable};
use crate::schema::*;

#[derive(Identifiable, Queryable, Debug)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub hashed_password: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub is_superadmin: bool,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}

#[derive(Insertable, Debug)]
#[diesel(table_name = users)]
pub struct InsertUser {
    pub email: String,
    pub hashed_password: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub is_superadmin: bool,
}
