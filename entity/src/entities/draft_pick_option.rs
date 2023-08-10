//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.2

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A Draft Pick Option is an additional clause that can be applied to a draft pick. They are first created in a trade proposal and become active when a trade is processed.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "draft_pick_option")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub clause: String,
    pub draft_pick_id: i64,
    pub status: DraftPickOptionStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// Because a draft pick option first exists as a trade idea and is not yet solidified/applied to a draft pick, the status field is used to describe this intermediary state.
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
pub enum DraftPickOptionStatus {
    /// The default status. This means the option has been proposed in a trade, but the trade has not been accepted yet.
    #[default]
    #[sea_orm(num_value = 0)]
    Proposed,
    /// The trade that created this option has been accepted and this option currently applies to the referenced draft pick.
    #[sea_orm(num_value = 1)]
    Active,
    /// The trade did not go through and this option died.
    #[sea_orm(num_value = 2)]
    Cancelled,
    /// This draft pick option has been activated / used on the draft pick.
    #[sea_orm(num_value = 3)]
    Used,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::draft_pick::Entity",
        from = "Column::DraftPickId",
        to = "super::draft_pick::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    DraftPick,
    #[sea_orm(has_many = "super::trade_asset::Entity")]
    TradeAsset,
}

impl Related<super::draft_pick::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DraftPick.def()
    }
}

impl Related<super::trade_asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAsset.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
