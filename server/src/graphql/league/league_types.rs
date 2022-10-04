use std::sync::Arc;

use async_graphql::{Context, Object, Result};
use fbkl_entity::{
    league, team_queries::find_teams_by_user, user, user_queries::find_users_by_league_id,
};

use crate::{
    graphql::{team::Team, user::User},
    server::AppState,
};
// use fbkl_entity::league;

#[derive(Default)]
pub struct League {
    pub id: i64,
    pub name: String,
    pub teams: Vec<Team>,
    pub users: Vec<User>,
}

impl League {
    pub fn from_model(entity: league::Model) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            teams: vec![],
            users: vec![],
        }
    }
}

#[Object]
impl League {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn teams(&self, ctx: &Context<'_>) -> Result<Vec<Team>> {
        let current_user = match ctx.data_unchecked::<Option<user::Model>>() {
            None => return Ok(vec![]),
            Some(user) => user,
        };
        let state = ctx.data_unchecked::<Arc<AppState>>();

        let user_team_models = find_teams_by_user(current_user, &state.db).await?;
        let user_teams = user_team_models
            .into_iter()
            .map(|team_model| {
                let mut team = Team::from_model(team_model);
                team.league = Some(League {
                    id: self.id,
                    name: self.name.clone(),
                    teams: vec![],
                    users: vec![],
                });

                team
            })
            .collect();

        Ok(user_teams)
    }

    async fn users(&self, ctx: &Context<'_>) -> Result<Vec<User>> {
        let state = ctx.data_unchecked::<Arc<AppState>>();
        let league_user_models = find_users_by_league_id(self.id, &state.db).await?;
        let league_users = league_user_models
            .into_iter()
            .map(User::from_model)
            .collect();

        Ok(league_users)
    }
}
