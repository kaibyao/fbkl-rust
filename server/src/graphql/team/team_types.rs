use async_graphql::Object;
use fbkl_entity::team;

use crate::graphql::{league::League, user::User};

use super::TeamUser;

#[derive(Clone, Default)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub league: Option<League>,
    pub league_id: i64,
    pub users: Option<Vec<TeamUser>>,
}

impl Team {
    pub fn from_model(entity: team::Model) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            league_id: entity.league_id,
            league: None,
            users: None,
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

    async fn league(&self) -> Option<League> {
        None
    }

    async fn league_id(&self) -> i64 {
        self.league_id
    }

    async fn users(&self) -> Option<Vec<User>> {
        None
    }
}
