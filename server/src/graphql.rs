// lint fires inside the MergedObject derive's own expansion below, not our code
#![allow(clippy::useless_let_if_seq)]

use async_graphql::{EmptySubscription, MergedObject, Schema};

use self::{
    contract::ContractQuery,
    keeper::{KeeperMutation, KeeperQuery},
    league::{LeagueMutation, LeagueQuery},
    player::PlayerQuery,
    roster::RosterMutation,
    team::TeamQuery,
    trade::{TradeMutation, TradeQuery},
    transaction::TransactionQuery,
    user::UserQuery,
};

pub use self::{authz::*, error::*};

mod authz;
mod contract;
mod error;
mod keeper;
mod league;
mod player;
mod roster;
mod team;
mod trade;
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
    TradeQuery,
    TransactionQuery,
    KeeperQuery,
);

#[derive(Default, MergedObject)]
pub struct MutationRoot(
    LeagueMutation,
    TradeMutation,
    RosterMutation,
    KeeperMutation,
);
