// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "token_type_enum"))]
    pub struct TokenTypeEnum;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::TokenTypeEnum;

    user_tokens (id) {
        id -> Int8,
        user_id -> Int8,
        token -> Bytea,
        token_type -> TokenTypeEnum,
        sent_to -> Nullable<Text>,
        inserted_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Int8,
        email -> Text,
        hashed_password -> Text,
        confirmed_at -> Nullable<Timestamptz>,
        is_superadmin -> Bool,
        inserted_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(user_tokens -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    user_tokens,
    users,
);
