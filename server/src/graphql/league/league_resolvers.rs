use super::League;
use async_graphql::{Context, Object, Result};
use fbkl_entity::{league_queries::find_leagues_by_user, sea_orm::DatabaseConnection, user};

#[derive(Default)]
pub struct LeagueQuery;

#[Object]
impl LeagueQuery {
    async fn leagues(&self, ctx: &Context<'_>) -> Result<Vec<League>> {
        let user_model = match ctx.data_unchecked::<Option<user::Model>>().to_owned() {
            None => return Ok(vec![]),
            Some(user) => user,
        };
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let league_models = find_leagues_by_user(&user_model, db).await?;

        let leagues = league_models.into_iter().map(League::from_model).collect();
        Ok(leagues)
    }
}
