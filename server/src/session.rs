use axum::http::StatusCode;
use fbkl_entity::{
    sea_orm::{DatabaseConnection, EntityTrait},
    user,
};
use tower_sessions::Session;

use crate::error::FbklError;

/// Used within a handler/resolver that checks if a user is currently logged in and if not, return an error.
pub async fn enforce_logged_in(session: Session) -> Result<i64, FbklError> {
    match session.get("user_id").await {
        Ok(Some(user_id)) => Ok(user_id),
        _ => Err(StatusCode::UNAUTHORIZED.into()),
    }
}

/// Used within a handler/resolver to get the current user from DB.
///
/// Distinguishes "no session / not logged in" (`Ok(None)`) from a real session-store
/// or DB failure (`Err`), so an outage can't masquerade as a logged-out user.
pub async fn get_current_user(
    session: Session,
    db: &DatabaseConnection,
) -> Result<Option<user::Model>, FbklError> {
    let Some(user_id) = session.get::<i64>("user_id").await? else {
        return Ok(None);
    };

    Ok(user::Entity::find_by_id(user_id).one(db).await?)
}
