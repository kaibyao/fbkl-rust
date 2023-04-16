//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "team_user")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_role: LeagueRole,
    pub nickname: String,
    /// The Season End Year that a user joined a league as an owner/commissioner.
    pub first_end_of_season_year: i16,
    /// The Season End Year that a user left a league.
    pub final_end_of_season_year: Option<i16>,
    pub team_id: i64,
    pub user_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

// A user's role (which determines access) in a league.
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
pub enum LeagueRole {
    /// Owns a team. Has access to their team's settings & roster.
    #[default]
    #[sea_orm(num_value = 0)]
    TeamOwner,
    /// Has access to a league's settings.
    #[sea_orm(num_value = 1)]
    LeagueCommissioner,
    /// User is inactive/deactivated in the league.
    #[sea_orm(num_value = 2)]
    Inactive,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::auction_bid::Entity")]
    Auction,
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(has_many = "super::trade_action::Entity")]
    TradeAction,
}

impl Related<super::auction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Auction.def()
    }
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl Related<super::trade_action::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAction.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn before_save(self, _insert: bool) -> Result<Self, DbErr> {
        validate_league_role(&self)?;
        Ok(self)
    }
}

fn validate_league_role(model: &ActiveModel) -> Result<(), DbErr> {
    match model.league_role.as_ref() {
        LeagueRole::Inactive => {
            if model.final_end_of_season_year.is_not_set()
                || model.final_end_of_season_year.as_ref().is_none()
            {
                return Err(DbErr::Custom(
                    "An inactive team user requires a final season year.".to_string(),
                ));
            }
        }
        _ => {
            if model.final_end_of_season_year.is_set()
                && model.final_end_of_season_year.as_ref().is_some()
            {
                return Err(DbErr::Custom(
                    "An active team user requires final season year to be unset.".to_string(),
                ));
            }
        }
    }

    Ok(())
}
