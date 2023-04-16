//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "draft_pick")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub protection_clause: Option<String>,
    pub round: i16,
    pub end_of_season_year: i16,
    pub league_id: i64,
    pub current_owner_team_id: i64,
    pub original_owner_team_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::draft_pick_option::Entity")]
    DraftPickOption,
    #[sea_orm(
        belongs_to = "super::league::Entity",
        from = "Column::LeagueId",
        to = "super::league::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    League,
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::CurrentOwnerTeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "NoAction"
    )]
    CurrentOwnerTeam,
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::OriginalOwnerTeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "NoAction"
    )]
    OriginalOwnerTeam,
    #[sea_orm(has_many = "super::rookie_draft_selection::Entity")]
    RookieDraftSelection,
    #[sea_orm(has_many = "super::trade_asset::Entity")]
    TradeAsset,
}

impl Related<super::draft_pick_option::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DraftPickOption.def()
    }
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

pub struct CurrentOwnerTeam;
impl Linked for CurrentOwnerTeam {
    type FromEntity = Entity;
    type ToEntity = super::team::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::CurrentOwnerTeam.def()]
    }
}

pub struct OriginalOwnerTeam;
impl Linked for OriginalOwnerTeam {
    type FromEntity = Entity;
    type ToEntity = super::team::Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::OriginalOwnerTeam.def()]
    }
}

impl Related<super::rookie_draft_selection::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RookieDraftSelection.def()
    }
}

impl Related<super::trade_asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAsset.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
