use crate::schema::*;
use chrono::{DateTime, Utc};
use diesel::{AsChangeset, Identifiable, Insertable, Queryable};
use serde::{Deserialize, Serialize};

#[derive(Identifiable, Queryable, Debug, Deserialize, Serialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub hashed_password: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub is_superadmin: bool,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = users)]
pub struct InsertUser {
    pub email: String,
    pub hashed_password: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub is_superadmin: bool,
}

#[derive(AsChangeset, Debug, Identifiable)]
#[diesel(table_name = users)]
pub struct UpdateUser {
    pub id: i64,
    pub email: Option<String>,
    pub hashed_password: Option<String>,
    pub confirmed_at: Option<Option<DateTime<Utc>>>,
    pub is_superadmin: Option<bool>,
}
