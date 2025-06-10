use crate::{
    error::FbklError,
    graphql::team::{Team, TeamUser},
    session::{enforce_logged_in, get_current_user},
};

use super::League;
use async_graphql::{Context, Error as GraphQlError, Object, Result};
use axum::http::StatusCode;
use fbkl_entity::{
    league_queries::{create_league_with_commissioner, find_league_by_user, find_leagues_by_user},
    sea_orm::DatabaseConnection,
    user,
};
use tower_sessions::Session;

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

    async fn league(&self, ctx: &Context<'_>) -> Result<League, FbklError> {
        let session = ctx.data_unchecked::<Session>();
        let selected_league_id: i64 = match session.get("selected_league_id").await? {
            None => return Err(StatusCode::BAD_REQUEST.into()),
            Some(id) => id,
        };

        let user_model = match ctx.data_unchecked::<Option<user::Model>>().to_owned() {
            None => return Err(StatusCode::UNAUTHORIZED.into()),
            Some(user) => user,
        };
        let db = ctx.data_unchecked::<DatabaseConnection>();

        match find_league_by_user(&user_model, selected_league_id, db).await? {
            None => Err(StatusCode::NOT_FOUND.into()),
            Some(league_model) => Ok(League::from_model(league_model)),
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
        let session = ctx.data_unchecked::<Session>();
        let user_id = enforce_logged_in(session.clone()).await?;

        let (league_model, team_model, team_user_model) =
            create_league_with_commissioner(league_name, team_name, user_id, user_nickname, db)
                .await?;

        let team_user = TeamUser {
            team_id: team_model.id,
            user_id,
            league_role: team_user_model.league_role,
            ..Default::default()
        };
        let mut team = Team::from_model(team_model);
        team.team_users = vec![team_user];
        let league = League::from_model(league_model);

        Ok(league)
    }

    async fn select_league(&self, ctx: &Context<'_>, league_id: i64) -> Result<League> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let session = ctx.data_unchecked::<Session>();

        let user_model = match get_current_user(session.clone(), db).await {
            None => return Err(GraphQlError::from(StatusCode::UNAUTHORIZED)),
            Some(model) => model,
        };

        // verify user has access to league
        match find_league_by_user(&user_model, league_id, db).await {
            // db error
            Err(_db_err) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
            Ok(None) => {
                // couldn't find league
                Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR))
            }
            // write league id to session
            Ok(Some(league_model)) => match session.insert("selected_league_id", league_id).await {
                Err(_) => Err(GraphQlError::from(StatusCode::INTERNAL_SERVER_ERROR)),
                Ok(_) => Ok(League::from_model(league_model)),
            },
        }
    }
}
