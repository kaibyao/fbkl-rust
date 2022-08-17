use chrono::{DateTime, Utc};
use diesel::{Identifiable, Queryable, Associations, Insertable};
use crate::schema::*;
use super::user_model::User;

#[derive(diesel_derive_enum::DbEnum, Debug)]
#[DieselTypePath = "crate::schema::sql_types::TokenTypeEnum"]
pub enum TokenTypeEnum {
    RegistrationConfirm,
    Session
}

#[derive(Identifiable, Queryable, Associations, Debug)]
#[diesel(table_name = user_tokens, belongs_to(User))]
pub struct UserToken {
    pub id: i64,
    pub user_id: i64,
    pub token: Vec<u8>,
    pub token_type: TokenTypeEnum,
    pub sent_to: Option<String>,
    pub inserted_at: DateTime<Utc>,
}

#[derive(Insertable, Associations, Debug)]
#[diesel(table_name = user_tokens, belongs_to(User))]
pub struct InsertUserToken {
    pub user_id: i64,
    pub token: Vec<u8>,
    pub token_type: TokenTypeEnum,
    pub sent_to: Option<String>,
}
