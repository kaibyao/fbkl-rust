//! The state of one restricted free agent's post-auction handshake (rules §15.3).
//!
//! A closed RFA auction signs nobody: the winner has 48h to raise, then the keeper-deadline owner
//! has 48h to match at a discount or decline. This row carries that state between the two windows,
//! including `original_owner_team_id` — the auction can move the contract's `team_id`, so the
//! discount right is snapshotted here instead of being re-derived later (§15.4.2).

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rfa_resolution")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub end_of_season_year: i16,
    /// The `RestrictedFreeAgent` contract being resolved.
    pub rfa_contract_id: i64,
    /// The team that held the player at the keeper deadline, which holds the discount right (rules §15.4.2).
    pub original_owner_team_id: i64,
    /// NULL when nobody bid — the raise/match handshake is skipped entirely (rules §15.3.5).
    pub auction_id: Option<i64>,
    pub winning_team_id: Option<i64>,
    pub final_bid: Option<i16>,
    /// When the winning bid was announced (rules §15.2.2). Picks the winner acquired after it cannot be forfeited.
    pub final_bid_at: Option<DateTimeWithTimeZone>,
    pub status: RfaResolutionStatus,
    /// The winner's optional raise, never below `final_bid` (rules §15.3.2.1).
    pub raised_bid: Option<i16>,
    /// Auction close + 48h.
    pub raise_deadline_at: DateTimeWithTimeZone,
    /// Set when the raise window resolves; that moment + 48h.
    pub match_deadline_at: Option<DateTimeWithTimeZone>,
    pub resolved_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// Where a restricted free agent sits in the raise/match handshake (rules §15.3).
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Enum, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum RfaResolutionStatus {
    /// Auction closed; the winner's 48h raise window is open.
    #[sea_orm(string_value = "AwaitingRaise")]
    AwaitingRaise,
    /// Raise window settled (raised or not); the original owner's 48h window is open.
    #[sea_orm(string_value = "AwaitingMatch")]
    AwaitingMatch,
    /// Original owner matched and re-signed the player at the discount.
    #[sea_orm(string_value = "Resolved")]
    Resolved,
    /// Original owner declined: the winner signs at the effective bid and forfeits a pick.
    #[sea_orm(string_value = "Declined")]
    Declined,
    /// Never bid on; original owner re-signed at the discounted 4th-year salary (rules §15.3.5).
    #[sea_orm(string_value = "NoBidResigned")]
    NoBidResigned,
    /// Never bid on; original owner passed, so the player goes to the regular auction (rules §15.3.5).
    #[sea_orm(string_value = "NoBidToAuction")]
    NoBidToAuction,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::league::Entity",
        from = "Column::LeagueId",
        to = "super::league::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    League,
    #[sea_orm(
        belongs_to = "super::contract::Entity",
        from = "Column::RfaContractId",
        to = "super::contract::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Contract,
    #[sea_orm(
        belongs_to = "super::auction::Entity",
        from = "Column::AuctionId",
        to = "super::auction::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Auction,
    #[sea_orm(has_many = "super::rfa_compensation_pick::Entity")]
    RfaCompensationPick,
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<super::contract::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contract.def()
    }
}

impl Related<super::auction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Auction.def()
    }
}

impl Related<super::rfa_compensation_pick::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RfaCompensationPick.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
