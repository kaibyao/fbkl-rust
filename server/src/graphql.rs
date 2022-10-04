use async_graphql::{EmptyMutation, EmptySubscription, MergedObject, Schema};

use self::{league::LeagueQuery, user::UserQuery};

mod league;
mod team;
mod user;

pub type FbklSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

#[derive(Default, MergedObject)]
pub struct QueryRoot(UserQuery, LeagueQuery);
