// lint fires inside the MergedObject derive's own expansion below, not our code
#![allow(clippy::useless_let_if_seq)]

use async_graphql::{EmptySubscription, MergedObject, Schema};

use self::{
    contract::ContractQuery,
    league::{LeagueMutation, LeagueQuery},
    player::PlayerQuery,
    team::TeamQuery,
    transaction::TransactionQuery,
    user::UserQuery,
};

pub use self::{authz::*, error::*};

mod authz;
mod contract;
mod error;
mod league;
mod player;
mod team;
mod transaction;
mod user;

pub type FbklSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

#[derive(Default, MergedObject)]
pub struct QueryRoot(
    UserQuery,
    LeagueQuery,
    TeamQuery,
    PlayerQuery,
    ContractQuery,
    TransactionQuery,
);

#[derive(Default, MergedObject)]
pub struct MutationRoot(LeagueMutation);
