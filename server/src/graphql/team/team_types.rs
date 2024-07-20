use async_graphql::{Context, Object, Result};
use fbkl_entity::{sea_orm::DatabaseConnection, team, team_user_queries::get_team_users_by_team};

use crate::graphql::league::League;

use super::TeamUser;

#[derive(Clone, Default)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub league: Option<League>,
    pub league_id: i64,
    pub team_users: Vec<TeamUser>,
}

impl Team {
    pub fn from_model(entity: team::Model) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            league_id: entity.league_id,
            league: None,
            team_users: vec![],
        }
    }
}

#[Object]
impl Team {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn league_id(&self) -> i64 {
        self.league_id
    }

    async fn league(&self) -> Option<League> {
        self.league.clone()
    }

    async fn team_users(&self, ctx: &Context<'_>) -> Result<Vec<TeamUser>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user_models = get_team_users_by_team(self.id, db).await?;

        let team_users = team_user_models
            .into_iter()
            .map(|team_user_model| TeamUser {
                league_role: team_user_model.league_role,
                nickname: team_user_model.nickname,
                team: None,
                team_id: team_user_model.team_id,
                user: None,
                user_id: team_user_model.user_id,
            })
            .collect();

        Ok(team_users)
    }
}
