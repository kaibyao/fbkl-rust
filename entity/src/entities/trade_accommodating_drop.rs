//! A move an owner submits with a trade so the trade fits their roster (rules §12.5.3, §13.1.4).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One contract an owner moves aside as part of a trade. `team_id` is the roster the contract sat
/// on when the move was submitted, which is whose transaction the move belongs to.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "trade_accommodating_drop")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub trade_id: i64,
    pub team_id: i64,
    pub contract_id: i64,
    pub kind: AccommodatingMoveKind,
}

/// Which move the owner declared, since rules §13.1.5.3 and §13.1.5.4 let a move to the IR make
/// room for a trade the way a drop does.
#[derive(
    Debug, Default, Clone, Copy, Eq, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum AccommodatingMoveKind {
    /// The contract leaves the roster.
    #[default]
    #[sea_orm(string_value = "Drop")]
    Drop,
    /// The contract goes to the injured reserve, freeing its active roster slot and cap space.
    #[sea_orm(string_value = "ToIr")]
    ToIr,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::trade::Entity",
        from = "Column::TradeId",
        to = "super::trade::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Trade,
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
    #[sea_orm(
        belongs_to = "super::contract::Entity",
        from = "Column::ContractId",
        to = "super::contract::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Contract,
}

impl ActiveModelBehavior for ActiveModel {}
