use super::User;
use crate::error::FbklError;
use async_graphql::{Context, Object};
use axum_sessions::extractors::ReadableSession;
use fbkl_entity::{
    sea_orm::{DatabaseConnection, EntityTrait},
    user,
};

#[derive(Default)]
pub struct UserQuery;

#[Object]
impl UserQuery {
    async fn current_user<'a>(&self, ctx: &Context<'a>) -> Result<Option<User>, FbklError> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<ReadableSession>();

        let user_model = match session.get("user_id") {
            Some(user_id) => user::Entity::find_by_id(user_id).one(db).await?,
            None => return Ok(None),
        };

        Ok(user_model.map(User::from_model))
    }
}
