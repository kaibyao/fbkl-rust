//! One row per drawn first-round lottery slot, picks 1-6 (rules §7.2.5).
//!
//! `balls_held` is what the team held at the moment of its draw, which is the audit trail for the
//! weighting. Picks 7-12 are deterministic from `playoff_finish` and are not stored here.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rookie_draft_lottery_pick")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub rookie_draft_lottery_id: i64,
    /// 1-6, the first-round pick this draw won.
    pub pick_number: i16,
    /// The non-playoff team whose standings slot won the draw; the pick itself goes to whoever
    /// currently owns that team's first-round pick.
    pub team_id: i64,
    pub balls_held: i16,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::rookie_draft_lottery::Entity",
        from = "Column::RookieDraftLotteryId",
        to = "super::rookie_draft_lottery::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    RookieDraftLottery,
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
}

impl Related<super::rookie_draft_lottery::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RookieDraftLottery.def()
    }
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
