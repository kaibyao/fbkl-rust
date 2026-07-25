// lint fires inside the MergedObject derive's own expansion below, not our code
#![allow(clippy::useless_let_if_seq)]

use async_graphql::{EmptySubscription, MergedObject, Schema};

use self::{
    league::{LeagueMutation, LeagueQuery},
    player::PlayerQuery,
    team::TeamQuery,
    user::UserQuery,
};

pub use self::{authz::*, error::*};

mod authz;
mod contract;
mod error;
mod league;
mod player;
mod team;
mod user;

pub type FbklSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

#[derive(Default, MergedObject)]
pub struct QueryRoot(UserQuery, LeagueQuery, TeamQuery, PlayerQuery);

#[derive(Default, MergedObject)]
pub struct MutationRoot(LeagueMutation);
