//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "team")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub league_id: i64,
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
    #[sea_orm(has_many = "super::team_user::Entity")]
    TeamUser,
    #[sea_orm(has_many = "super::team_update::Entity")]
    TeamUpdate,
    #[sea_orm(has_many = "super::trade_asset::Entity")]
    TradeAsset,
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

impl Related<super::team_user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TeamUser.def()
    }
}

impl Related<super::team_update::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TeamUpdate.def()
    }
}

impl Related<super::trade_asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAsset.def()
    }
}

#[derive(Debug)]
pub struct TradesProposed;
impl Linked for TradesProposed {
    type FromEntity = Entity;
    type ToEntity = super::trade::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![super::trade::Relation::FromTeam.def().rev()]
    }
}

#[derive(Debug)]
pub struct TradesResponded;
impl Linked for TradesResponded {
    type FromEntity = Entity;
    type ToEntity = super::trade::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![super::trade::Relation::ToTeam.def().rev()]
    }
}

impl ActiveModelBehavior for ActiveModel {}
