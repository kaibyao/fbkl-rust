use axum::http::StatusCode;
use axum_sessions::extractors::{ReadableSession, WritableSession};
use fbkl_entity::{
    sea_orm::{DatabaseConnection, EntityTrait},
    user,
};

/// Used within a handler/resolver that checks if a user is currently logged in and if not, return an error.
pub fn enforce_logged_in(session: &ReadableSession) -> Result<i64, StatusCode> {
    match session.get("user_id") {
        Some(user_id) => Ok(user_id),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Used within a handler/resolver to get the current user from DB.
pub async fn get_current_user(
    session: &ReadableSession,
    db: &DatabaseConnection,
) -> Option<user::Model> {
    let user_id: i64 = match session.get("user_id") {
        None => return None,
        Some(user_id) => user_id,
    };

    match user::Entity::find_by_id(user_id).one(db).await {
        Err(_) => None,
        Ok(maybe_user) => maybe_user,
    }
}

pub async fn get_current_user_writable(
    session: &WritableSession,
    db: &DatabaseConnection,
) -> Option<user::Model> {
    let user_id: i64 = match session.get("user_id") {
        None => return None,
        Some(user_id) => user_id,
    };

    match user::Entity::find_by_id(user_id).one(db).await {
        Err(_) => None,
        Ok(maybe_user) => maybe_user,
    }
}
