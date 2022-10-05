use async_graphql::Object;
use fbkl_entity::team_user::LeagueRole;

use crate::graphql::user::User;

use super::Team;

#[derive(Clone, Default)]
pub struct TeamUser {
    pub team: Option<Team>,
    pub team_id: i64,
    pub user: Option<User>,
    pub user_id: i64,
    pub league_role: LeagueRole,
}

#[Object]
impl TeamUser {
    async fn team(&self) -> Option<Team> {
        self.team.clone()
    }

    async fn team_id(&self) -> i64 {
        self.team_id
    }

    async fn user(&self) -> Option<User> {
        self.user.clone()
    }

    async fn user_id(&self) -> i64 {
        self.user_id
    }

    async fn league_role(&self) -> LeagueRole {
        self.league_role
    }
}
