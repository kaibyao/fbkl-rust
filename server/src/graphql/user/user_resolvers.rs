use super::User;
use async_graphql::{Context, Object};
use fbkl_entity::user;

#[derive(Default)]
pub struct UserQuery;

#[Object]
impl UserQuery {
    async fn current_user<'a>(&self, ctx: &Context<'a>) -> Option<User> {
        let user_model = ctx.data_unchecked::<Option<user::Model>>().to_owned();

        user_model.map(User::from_model)
    }
}
