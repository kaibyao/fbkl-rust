//! A drop an owner submits with a trade so the trade fits their roster (rules §12.5.3, §13.1.4).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One contract an owner drops as part of a trade. `team_id` is the roster the contract sat on when
/// the drop was submitted, which is whose transaction the drop belongs to.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "trade_accommodating_drop")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub trade_id: i64,
    pub team_id: i64,
    pub contract_id: i64,
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
