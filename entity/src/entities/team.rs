//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use std::fmt::Debug;

use color_eyre::{eyre::eyre, Result};
use sea_orm::{entity::prelude::*, ConnectionTrait};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    contract::{self, ContractStatus},
    league, team_user,
};

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

impl Model {
    #[instrument]
    pub async fn get_league<C>(&self, db: &C) -> Result<league::Model>
    where
        C: ConnectionTrait + Debug,
    {
        let league_model = self.find_related(league::Entity).one(db).await?;
        league_model.ok_or_else(|| {
            eyre!(
                "Could not find league related to team model with id: {}.",
                self.id
            )
        })
    }

    pub async fn get_team_users<C>(&self, db: &C) -> Result<Vec<team_user::Model>>
    where
        C: ConnectionTrait + Debug,
    {
        let team_user_models = self.find_related(team_user::Entity).all(db).await?;
        Ok(team_user_models)
    }

    /// Retrieves all active contracts for a given team.
    pub async fn get_active_contracts<C>(&self, db: &C) -> Result<Vec<contract::Model>>
    where
        C: ConnectionTrait + Debug,
    {
        let active_team_contracts = self
            .find_related(contract::Entity)
            .filter(contract::Column::Status.eq(ContractStatus::Active))
            .all(db)
            .await?;
        Ok(active_team_contracts)
    }
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
    #[sea_orm(has_many = "super::trade::Entity")]
    Trade,
    #[sea_orm(has_many = "super::team_user::Entity")]
    TeamUser,
    #[sea_orm(has_many = "super::team_update::Entity")]
    TeamUpdate,
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

impl Related<super::trade::Entity> for Entity {
    // The final relation is Team -> TeamTrade -> Trade
    fn to() -> RelationDef {
        super::team_trade::Relation::Trade.def()
    }

    fn via() -> Option<RelationDef> {
        // The original relation is TeamTrade -> Team,
        // after `rev` it becomes Team -> TeamTrade
        Some(super::team_trade::Relation::Team.def().rev())
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

impl Related<super::user::Entity> for Entity {
    // The final relation is Team -> TeamUser -> User
    fn to() -> RelationDef {
        team_user::Relation::User.def()
    }

    fn via() -> Option<RelationDef> {
        // The original relation is TeamUser -> Team,
        // after `rev` it becomes Team -> TeamUser
        Some(team_user::Relation::Team.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
