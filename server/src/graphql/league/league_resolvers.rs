use crate::{
    error::FbklError,
    graphql::team::{Team, TeamUser},
    session::enforce_logged_in,
};

use super::League;
use async_graphql::{Context, Object, Result};
use axum::http::StatusCode;
use axum_sessions::extractors::ReadableSession;
use fbkl_entity::{
    league,
    league_queries::{create_league, find_leagues_by_user},
    sea_orm::DatabaseConnection,
    user,
};

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

    async fn league(&self, ctx: &Context<'_>, id: i64) -> Result<League, FbklError> {
        let user_model = match ctx.data_unchecked::<Option<user::Model>>().to_owned() {
            None => return Err(StatusCode::NOT_FOUND.into()),
            Some(user) => user,
        };
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let user_league_models = find_leagues_by_user(&user_model, db)
            .await?
            .into_iter()
            .filter(|league_model| league_model.id == id)
            .collect::<Vec<league::Model>>();
        match user_league_models.first() {
            None => Err(StatusCode::NOT_FOUND.into()),
            Some(league_model) => Ok(League::from_model(league_model.to_owned())),
        }
    }
}

#[derive(Default)]
pub struct LeagueMutation;

#[Object]
impl LeagueMutation {
    async fn create_league(
        &self,
        ctx: &Context<'_>,
        league_name: String,
        team_name: String,
        user_nickname: String,
    ) -> Result<League> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<ReadableSession>();
        let user_id = enforce_logged_in(session)?;

        let (league_model, team_model, team_user_model) =
            create_league(league_name, team_name, user_id, user_nickname, db).await?;

        let team_user = TeamUser {
            team_id: team_model.id,
            user_id,
            league_role: team_user_model.league_role,
            ..Default::default()
        };
        let mut team = Team::from_model(team_model);
        team.team_users = Some(vec![team_user]);
        let mut league = League::from_model(league_model);
        league.teams = vec![team];

        Ok(league)
    }
}
