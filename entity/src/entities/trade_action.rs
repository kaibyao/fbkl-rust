//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.2

use std::fmt::Debug;

use async_graphql::Enum;
use color_eyre::Result;
use sea_orm::{entity::prelude::*, ActiveValue};
use serde::{Deserialize, Serialize};

use crate::team_user;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "trade_action")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub action_type: TradeActionType,
    pub team_user_id: i64,
    pub trade_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl Model {
    pub fn new_active_model(
        trade_action_type: TradeActionType,
        trade_id: i64,
        team_user_id: i64,
    ) -> ActiveModel {
        ActiveModel {
            id: ActiveValue::NotSet,
            action_type: ActiveValue::Set(trade_action_type),
            team_user_id: ActiveValue::Set(team_user_id),
            trade_id: ActiveValue::Set(trade_id),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        }
    }
}

/// Represents the different types of actions that can be made by a team involved in a trade.
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
pub enum TradeActionType {
    /// A trade is proposed by a team.
    #[default]
    #[sea_orm(num_value = 0)]
    Propose,
    /// A trade has been accepted by the responding team.
    #[sea_orm(num_value = 1)]
    Accept,
    /// A trade has been canceled by the proposing team. If a trade has been canceled, no other action can be taken on it.
    #[sea_orm(num_value = 2)]
    Cancel,
    /// A trade has been rejected by the responding team.
    #[sea_orm(num_value = 3)]
    Reject,
    /// A trade has been counter-offered by the responding team. Once a trade has been countered, no other action can be taken on the now-obsolete version of the trade.
    #[sea_orm(num_value = 4)]
    Counteroffer,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::team_user::Entity",
        from = "Column::TeamUserId",
        to = "super::team_user::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    TeamUser,
    #[sea_orm(
        belongs_to = "super::trade::Entity",
        from = "Column::TradeId",
        to = "super::trade::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Trade,
}

impl Related<super::team_user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TeamUser.def()
    }
}

impl Related<super::team::Entity> for Entity {
    // The final relation is Trade Action -> TeamUser -> Team
    fn to() -> RelationDef {
        team_user::Relation::Team.def()
    }

    fn via() -> Option<RelationDef> {
        // The original relation is TeamUser -> Trade Action,
        // after `rev` it becomes Trade Action -> TeamUser
        Some(team_user::Relation::TradeAction.def().rev())
    }
}

impl Related<super::trade::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Trade.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
