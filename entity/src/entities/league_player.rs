//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.2

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A Basketball player customly created for a league. Reasons for this happening include:
/// * A basketball player being drafted but not active in the NBA.
/// * ...that's really about it.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "league_player")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub end_of_season_year: i16,
    pub league_id: i64,
    /// This id field gets filled when a custom-created player for a specific league eventually gets added to an official (NBA/ESPN) database. This is used to tie in a player's historical record in a league to before they became an NBA player.
    pub real_player_id: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contract::Entity")]
    Contract,
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
        from = "Column::RealPlayerId",
        to = "super::player::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Player,
}

impl Related<super::contract::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contract.def()
    }
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
