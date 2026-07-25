//! One row per league season recording the rookie draft lottery draw (rules §7.2.4-§7.2.5).
//!
//! `rng_seed` is stored when the row is created, before the draw is revealed (commit-reveal), so
//! owners can re-run the draw themselves and confirm it was not re-rolled. `rng_log` is the
//! human-readable per-draw record of who held how many balls.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rookie_draft_lottery")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub end_of_season_year: i16,
    /// Seed handed to `StdRng::seed_from_u64`, committed before the draw.
    pub rng_seed: i64,
    pub rng_log: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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
    #[sea_orm(has_many = "super::rookie_draft_lottery_pick::Entity")]
    RookieDraftLotteryPick,
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<super::rookie_draft_lottery_pick::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RookieDraftLotteryPick.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
