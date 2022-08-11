table! {
    use diesel::sql_types::*;

    user_tokens (id) {
        id -> Int8,
        user_id -> Int8,
        token -> Text,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;
    use diesel_citext::sql_types::*;

    users (id) {
        id -> Int8,
        email -> Citext,
        hashed_password -> Text,
        confirmed_at -> Nullable<Timestamptz>,
        is_superadmin -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

joinable!(user_tokens -> users (user_id));

allow_tables_to_appear_in_same_query!(
    user_tokens,
    users,
);
