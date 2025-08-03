use async_graphql::{EmptySubscription, MergedObject, Schema};

use self::{
    deadline_config::{DeadlineConfigMutation, DeadlineConfigQuery},
    league::{LeagueMutation, LeagueQuery},
    user::UserQuery,
};

mod contract;
mod deadline_config;
mod league;
mod player;
mod team;
mod user;

pub type FbklSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

#[derive(Default, MergedObject)]
pub struct QueryRoot(UserQuery, LeagueQuery, DeadlineConfigQuery);

#[derive(Default, MergedObject)]
pub struct MutationRoot(LeagueMutation, DeadlineConfigMutation);
