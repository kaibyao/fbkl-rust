use chrono::{DateTime, Utc};
use diesel::{Identifiable, Queryable, Associations};
use super::schema::*;

#[derive(Identifiable, Queryable, Debug)]
#[table_name = "users"]
pub struct User {
    pub id: i32,
    pub email: String,
    pub hashed_password: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub is_superadmin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}

#[derive(Identifiable, Queryable, Associations, Debug)]
#[belongs_to(User)]
#[table_name = "user_tokens"]
pub struct UserToken {
    pub id: i32,
    pub user_id: i32,
    pub token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>
}
