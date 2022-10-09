use async_graphql::{Context, Object, Result};
use fbkl_entity::{
    league, sea_orm::DatabaseConnection, team_queries::find_teams_by_user,
    team_user_queries::get_team_user_by_user_and_league, user,
};

use crate::graphql::{
    team::{Team, TeamUser},
    user::User,
};

#[derive(Clone, Default)]
pub struct League {
    pub id: i64,
    pub name: String,
    pub teams: Vec<Team>,
    pub current_team_user: Option<Box<TeamUser>>,
}

impl League {
    pub fn from_model(league_model: league::Model) -> Self {
        Self {
            id: league_model.id,
            name: league_model.name,
            teams: vec![],
            ..Default::default()
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
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let user_team_models = find_teams_by_user(current_user, db).await?;
        let user_teams = user_team_models
            .into_iter()
            .map(|team_model| {
                let mut team = Team::from_model(team_model);
                team.league = Some(League {
                    id: self.id,
                    name: self.name.clone(),
                    teams: vec![],
                    current_team_user: None,
                });

                team
            })
            .collect();

        Ok(user_teams)
    }

    async fn current_team_user(&self, ctx: &Context<'_>) -> Option<Box<TeamUser>> {
        let current_user = match ctx.data_unchecked::<Option<user::Model>>() {
            None => return None,
            Some(user) => user,
        };

        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user_model =
            match get_team_user_by_user_and_league(&current_user.id, &self.id, db).await {
                Err(_) => return None,
                Ok(maybe_team_user) => match maybe_team_user {
                    None => return None,
                    Some(team_user) => team_user,
                },
            };

        Some(Box::new(TeamUser {
            league_role: team_user_model.league_role,
            nickname: team_user_model.nickname,
            team: None,
            team_id: team_user_model.team_id,
            user: Some(User::from_model(current_user)),
            user_id: current_user.id,
        }))
    }
}
