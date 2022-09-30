//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "player")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub photo_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub position_id: i64,
    pub real_team_id: i64,
    pub espn_id: Option<i32>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::position::Entity",
        from = "Column::PositionId",
        to = "super::position::Column::Id",
        on_update = "Cascade",
        on_delete = "NoAction"
    )]
    Position,
    #[sea_orm(
        belongs_to = "super::real_team::Entity",
        from = "Column::RealTeamId",
        to = "super::real_team::Column::Id",
        on_update = "Cascade",
        on_delete = "NoAction"
    )]
    RealTeam,
}

impl Related<super::position::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Position.def()
    }
}

impl Related<super::real_team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RealTeam.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}