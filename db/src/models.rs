use chrono::{DateTime, Utc};
use diesel::{Identifiable, Queryable, Associations, Insertable};
use super::schema::*;

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
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}

#[derive(diesel_derive_enum::DbEnum, Debug)]
#[DieselTypePath = "crate::schema::sql_types::TokenTypeEnum"]
pub enum TokenTypeEnum {
    RegistrationConfirm,
    Session
}

#[derive(Identifiable, Insertable, Queryable, Associations, Debug)]
#[diesel(table_name = user_tokens, belongs_to(User))]
pub struct UserToken {
    pub id: i64,
    pub user_id: i64,
    pub token: Vec<u8>,
    pub token_type: TokenTypeEnum,
    pub sent_to: Option<String>,
    pub inserted_at: DateTime<Utc>,
}
