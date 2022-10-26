//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "team_update")]
pub struct Model {
    #[serde(skip_deserializing)]
    #[sea_orm(primary_key)]
    pub id: i64,
    pub team_id: i64,
    pub status: TeamUpdateStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Enum,
    Eq,
    PartialEq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
)]
#[sea_orm(rs_type = "i16", db_type = "Integer")]
pub enum TeamUpdateStatus {
    /// Has not been processed yet.
    #[default]
    #[sea_orm(num_value = 0)]
    Pending,
    /// Is currently being processed.
    #[sea_orm(num_value = 1)]
    InProgress,
    /// Has finished processing.
    #[sea_orm(num_value = 2)]
    Done,
    /// An error occurred during processing.
    #[sea_orm(num_value = 3)]
    Error,
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
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}