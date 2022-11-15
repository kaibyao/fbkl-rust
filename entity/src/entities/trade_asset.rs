//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.2

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "trade_asset")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub asset_type: TradeAssetType,
    pub draft_pick_condition: Option<String>,
    pub player_name_at_time_of_trade: Option<String>,
    pub player_team_name_at_time_of_trade: Option<String>,
    pub contract_id: Option<i64>,
    pub draft_pick_id: Option<i64>,
    pub from_team_id: i64,
    pub player_position_id_at_time_of_trade: Option<i64>,
    pub trade_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// Represents the different types of assets (contracts, draft picks, etc.) that can be traded.
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
pub enum TradeAssetType {
    #[default]
    #[sea_orm(num_value = 0)]
    Contract,
    #[sea_orm(num_value = 1)]
    DraftPick,
    #[sea_orm(num_value = 2)]
    DraftPickOption,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contract::Entity",
        from = "Column::ContractId",
        to = "super::contract::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Contract,
    #[sea_orm(
        belongs_to = "super::draft_pick::Entity",
        from = "Column::DraftPickId",
        to = "super::draft_pick::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    DraftPick,
    #[sea_orm(
        belongs_to = "super::position::Entity",
        from = "Column::PlayerPositionIdAtTimeOfTrade",
        to = "super::position::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Position,
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::FromTeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
    #[sea_orm(
        belongs_to = "super::trade::Entity",
        from = "Column::TradeId",
        to = "super::trade::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Trade,
}

impl Related<super::contract::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contract.def()
    }
}

impl Related<super::draft_pick::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DraftPick.def()
    }
}

impl Related<super::position::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Position.def()
    }
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl Related<super::trade::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Trade.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
