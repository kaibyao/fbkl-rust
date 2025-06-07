use async_graphql::{EmptySubscription, MergedObject, Schema};

use self::{
    league::{LeagueMutation, LeagueQuery},
    user::UserQuery,
};

mod contract;
mod league;
mod player;
mod team;
mod user;

pub type FbklSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

#[derive(Default, MergedObject)]
pub struct QueryRoot(UserQuery, LeagueQuery);

#[derive(Default, MergedObject)]
pub struct MutationRoot(LeagueMutation);
