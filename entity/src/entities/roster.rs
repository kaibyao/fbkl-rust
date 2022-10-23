//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "roster")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub salary_cap: i16,
    pub season_end_year: i16,
    pub team_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
    #[sea_orm(has_many = "super::roster_update::Entity")]
    RosterUpdate,
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl Related<super::roster_update::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RosterUpdate.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
