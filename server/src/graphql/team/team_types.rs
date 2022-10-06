use async_graphql::Object;
use fbkl_entity::team;

use crate::graphql::league::League;

use super::TeamUser;

#[derive(Clone, Default)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub league: Option<League>,
    pub league_id: i64,
    pub team_users: Option<Vec<TeamUser>>,
}

impl Team {
    pub fn from_model(entity: team::Model) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            league_id: entity.league_id,
            league: None,
            team_users: None,
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

    async fn team_users(&self) -> Option<Vec<TeamUser>> {
        self.team_users.clone()
    }
}
