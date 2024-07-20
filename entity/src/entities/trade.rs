//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.2

use std::fmt::Debug;

use async_graphql::Enum;
use async_trait::async_trait;
use color_eyre::{eyre::eyre, Result};
use sea_orm::{entity::prelude::*, ConnectionTrait};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{team, trade_action, trade_asset};

/// Trades made between 2 or more teams can be proposed, accepted, counteroffered, canceled, or rejected. When a trade is counteroffered, a new trade is created that refers to the previous. In this way, a historical chain of record can be made.
///
/// Note: Use trade_model.find_linked(TeamsInvolvedInTrade) and team_model.find_linked(TradesInvolvedByTeam) rather than .find_related(), as we use that many-to-many relationship to allow for multi-team trades to happen.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "trade")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub end_of_season_year: i16,
    pub status: TradeStatus,
    pub league_id: i64,
    pub original_trade_id: Option<i64>,
    pub previous_trade_id: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl Model {
    #[instrument]
    pub async fn get_trade_actions<C>(&self, db: &C) -> Result<Vec<super::trade_action::Model>>
    where
        C: ConnectionTrait + Debug,
    {
        let trade_actions = self.find_related(trade_action::Entity).all(db).await?;
        Ok(trade_actions)
    }

    #[instrument]
    pub async fn get_trade_assets<C>(&self, db: &C) -> Result<Vec<super::trade_asset::Model>>
    where
        C: ConnectionTrait + Debug,
    {
        let trade_assets = self.find_related(trade_asset::Entity).all(db).await?;
        Ok(trade_assets)
    }

    #[instrument]
    pub async fn get_teams<C>(&self, db: &C) -> Result<Vec<super::team::Model>>
    where
        C: ConnectionTrait + Debug,
    {
        let teams = self.find_related(team::Entity).all(db).await?;
        Ok(teams)
    }

    pub fn is_active(&self) -> bool {
        self.status == TradeStatus::Proposed || self.status == TradeStatus::Counteroffered
    }

    #[instrument]
    pub async fn is_latest_in_chain<C>(&self, db: &C) -> Result<bool>
    where
        C: ConnectionTrait + Debug,
    {
        let mut all_trades_in_chain = Entity::find()
            .filter(Column::OriginalTradeId.eq(self.original_trade_id))
            .all(db)
            .await?;
        all_trades_in_chain.sort_by(|a, b| a.id.cmp(&b.id));
        all_trades_in_chain
            .last()
            .map(|last_trade_in_chain| last_trade_in_chain.id == self.id)
            .ok_or_else(|| {
                let trade_ids_in_chain: Vec<String> = all_trades_in_chain
                    .iter()
                    .map(|trade| trade.id.to_string())
                    .collect();
                eyre!(
                    "Could not retrieve last trade in trade chain: [{}]",
                    trade_ids_in_chain.join(", ")
                )
            })
    }
}

/// Represents the different types of contract to which a player can be signed. When a player is signed to a team, their contract must be of one of these types.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Enum,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum TradeStatus {
    /// Trade has been proposed (default).
    #[default]
    #[sea_orm(string_value = "Proposed")]
    Proposed,
    /// Trade has been accepted and processed.
    #[sea_orm(string_value = "Completed")]
    Completed,
    /// Trade has been canceled by the proposing team.
    #[sea_orm(string_value = "Canceled")]
    Canceled,
    /// Trade has been rejected by a responding team.
    #[sea_orm(string_value = "Rejected")]
    Rejected,
    /// Trade has been counter-offered by a responding team.
    #[sea_orm(string_value = "Counteroffered")]
    Counteroffered,
    /// Trade has been invalidated by another trade that was processed that involves any of the offered assets.
    #[sea_orm(string_value = "InvalidatedByExternalTrade")]
    InvalidatedByExternalTrade,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::league::Entity",
        from = "Column::LeagueId",
        to = "super::league::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    League,
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::OriginalTradeId",
        to = "Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    OriginalTrade,
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::PreviousTradeId",
        to = "Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    PreviousTrade,
    #[sea_orm(has_many = "super::team::Entity")]
    Team,
    #[sea_orm(has_many = "super::trade_action::Entity")]
    TradeAction,
    #[sea_orm(has_many = "super::trade_asset::Entity")]
    TradeAsset,
    #[sea_orm(has_one = "super::transaction::Entity")]
    Transaction,
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<super::team::Entity> for Entity {
    // The final relation is Trade -> TeamTrade -> Team
    fn to() -> RelationDef {
        super::team_trade::Relation::Team.def()
    }

    fn via() -> Option<RelationDef> {
        // The original relation is TeamTrade -> Trade,
        // after `rev` it becomes Trade -> TeamTrade
        Some(super::team_trade::Relation::Trade.def().rev())
    }
}

impl Related<super::trade_action::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAction.def()
    }
}

impl Related<super::trade_asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAsset.def()
    }
}

impl Related<super::transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Transaction.def()
    }
}

#[derive(Debug)]
pub struct OriginalTrade;
impl Linked for OriginalTrade {
    type FromEntity = Entity;
    type ToEntity = Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::OriginalTrade.def()]
    }
}

#[derive(Debug)]
pub struct PreviousTrade;
impl Linked for PreviousTrade {
    type FromEntity = Entity;
    type ToEntity = Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::PreviousTrade.def()]
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        non_original_trade_requires_previous_trade(&self)?;
        original_trade_requires_unset_previous_trade(&self)?;

        if !insert {
            update_requires_original_trade(&self)?;
        }

        Ok(self)
    }
}

fn non_original_trade_requires_previous_trade(model: &ActiveModel) -> Result<(), DbErr> {
    if model.previous_trade_id.is_not_set()
        && model.original_trade_id.is_set()
        && model.original_trade_id.as_ref().as_ref().unwrap() != model.id.as_ref()
    {
        Err(DbErr::Custom(format!("This trade (id={}, original_trade_id={:?}) is missing a reference to the previous trade for this player.", model.id.as_ref(), model.original_trade_id.as_ref())))
    } else {
        Ok(())
    }
}

fn original_trade_requires_unset_previous_trade(model: &ActiveModel) -> Result<(), DbErr> {
    if model.previous_trade_id.is_set()
        && model.original_trade_id.is_set()
        && model.original_trade_id.as_ref().as_ref().unwrap() == model.id.as_ref()
    {
        Err(DbErr::Custom(format!("This trade (id={}, original_trade_id={:?}, previous_trade_id={:?}) is supposedly the original (id and original id are matching), yet a previous trade id is referenced.", model.id.as_ref(), model.original_trade_id.as_ref(), model.previous_trade_id.as_ref())))
    } else {
        Ok(())
    }
}

fn update_requires_original_trade(model: &ActiveModel) -> Result<(), DbErr> {
    if model.original_trade_id.is_not_set() {
        Err(DbErr::Custom(format!(
            "This trade (id={}) requires original_trade_id to be set before it can be saved.",
            model.id.as_ref()
        )))
    } else {
        Ok(())
    }
}
