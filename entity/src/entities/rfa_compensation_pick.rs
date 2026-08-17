//! The rookie-draft pick a declined RFA costs the winning team (rules §15.2).
//!
//! `required_round` is the tier the current bid landed in; the bidder may hand over that round or
//! any earlier one. Every bid on a restricted free agent names its pick as it is placed (rules
//! §15.3.3), so this row exists from the first bid and each later bid, raise or swap rewrites it:
//! it always says what the team currently leading would forfeit. A matched RFA leaves it behind as
//! the record of a debt that never came due. The pick itself moves only on a decline, by rewriting
//! `draft_pick.current_owner_team_id`, the same way a trade moves it.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rfa_compensation_pick")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub rfa_resolution_id: i64,
    /// Highest round number the compensation may be, from the bid tier table (rules §15.2.1).
    pub required_round: i16,
    /// The pick the bidder named. It changes hands only if the original owner declines.
    pub forfeited_draft_pick_id: i64,
    /// The original owner, who receives the pick.
    pub to_team_id: i64,
    /// The team currently leading the bid, which gives up the pick.
    pub from_team_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::rfa_resolution::Entity",
        from = "Column::RfaResolutionId",
        to = "super::rfa_resolution::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    RfaResolution,
    #[sea_orm(
        belongs_to = "super::draft_pick::Entity",
        from = "Column::ForfeitedDraftPickId",
        to = "super::draft_pick::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    DraftPick,
}

impl Related<super::rfa_resolution::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RfaResolution.def()
    }
}

impl Related<super::draft_pick::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DraftPick.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
