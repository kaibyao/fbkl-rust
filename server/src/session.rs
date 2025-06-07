use axum::http::StatusCode;
use fbkl_entity::{
    sea_orm::{DatabaseConnection, EntityTrait},
    user,
};
use tower_sessions::Session;

/// Used within a handler/resolver that checks if a user is currently logged in and if not, return an error.
pub async fn enforce_logged_in(session: Session) -> Result<i64, StatusCode> {
    match session.get("user_id").await {
        Ok(Some(user_id)) => Ok(user_id),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Used within a handler/resolver to get the current user from DB.
pub async fn get_current_user(session: Session, db: &DatabaseConnection) -> Option<user::Model> {
    let user_id: i64 = match session.get("user_id").await {
        Ok(Some(user_id)) => user_id,
        _ => return None,
    };

    (user::Entity::find_by_id(user_id).one(db).await).unwrap_or_default()
}
