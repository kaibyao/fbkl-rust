// lint fires inside the MergedObject derive's own expansion below, not our code
#![allow(clippy::useless_let_if_seq)]

use async_graphql::{EmptySubscription, MergedObject, Schema};

use self::{
    auction::{AuctionMutation, AuctionQuery},
    contract::ContractQuery,
    deadline::{DeadlineMutation, DeadlineQuery},
    draft::{DraftMutation, DraftQuery},
    eligibility::{EligibilityMutation, EligibilityQuery},
    keeper::{KeeperMutation, KeeperQuery},
    league::{LeagueMutation, LeagueQuery},
    league_event::LeagueEventQuery,
    player::PlayerQuery,
    rfa::{RfaMutation, RfaQuery},
    roster::{RosterMutation, RosterQuery},
    team::TeamQuery,
    trade::{TradeMutation, TradeQuery},
    user::UserQuery,
};

pub use self::{authz::*, error::*, loaders::*, season::*};

mod auction;
mod authz;
mod contract;
mod deadline;
mod draft;
mod eligibility;
mod error;
mod keeper;
mod league;
mod league_event;
mod loaders;
mod player;
mod rfa;
mod roster;
mod season;
mod team;
mod trade;
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
    LeagueEventQuery,
    KeeperQuery,
    DeadlineQuery,
    AuctionQuery,
    DraftQuery,
    EligibilityQuery,
    RfaQuery,
    RosterQuery,
);

#[derive(Default, MergedObject)]
pub struct MutationRoot(
    LeagueMutation,
    TradeMutation,
    RosterMutation,
    KeeperMutation,
    DeadlineMutation,
    EligibilityMutation,
    AuctionMutation,
    DraftMutation,
    RfaMutation,
);
