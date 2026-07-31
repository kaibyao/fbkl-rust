//! A season's ordered minimum-bid tiers for the veteran auction (rules §6.3.6).
//!
//! Commissioner input, one row per tier. `tier_index` 0 is the top tier; an unbid auction slides
//! down to the next configured tier's `min_bid_amount` per §6.3.4-.5.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "min_bid_tier_config")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub end_of_season_year: i16,
    pub tier_index: i16,
    pub min_bid_amount: i16,
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
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
