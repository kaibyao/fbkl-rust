//! One row per player in a season's veteran auction pool (rules §6.3).
//!
//! The daily release tick opens auctions for rows whose `scheduled_release_date` has arrived.
//! Ranked (top-150) players get an explicit `nomination_rank` and a staggered release date;
//! everyone else is open-nomination (`nomination_rank` NULL). `min_bid_tier` indexes into
//! `min_bid_tier_config` for the opening minimum bid, which slides down a tier per §6.3.4 while
//! the auction goes unbid.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auction_schedule")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub end_of_season_year: i16,
    pub player_id: i64,
    pub scheduled_release_date: Date,
    /// Position in the season's ranked top-150 list. NULL for open-nomination players.
    pub nomination_rank: Option<i16>,
    /// `min_bid_tier_config.tier_index` this player opens at.
    pub min_bid_tier: i16,
    /// Whether this player is released during RFA week (rules §6.2.2).
    pub is_rfa_week: bool,
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
    #[sea_orm(
        belongs_to = "super::player::Entity",
        from = "Column::PlayerId",
        to = "super::player::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Player,
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<super::player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
