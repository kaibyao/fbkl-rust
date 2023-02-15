//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use crate::contract;
use crate::team_user;
use async_graphql::Enum;
use color_eyre::eyre::Error;
use sea_orm::entity::prelude::*;
use sea_orm::ConnectionTrait;
use sea_orm::QuerySelect;
use sea_orm::TransactionTrait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "team_update")]
pub struct Model {
    #[serde(skip_deserializing)]
    #[sea_orm(primary_key)]
    pub id: i64,
    pub update_type: TeamUpdateType,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub effective_date: Date,
    pub status: TeamUpdateStatus,
    pub team_id: i64,
    pub transaction_id: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

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
pub enum TeamUpdateStatus {
    /// Has not been processed yet.
    #[default]
    #[sea_orm(num_value = 0)]
    Pending,
    /// Is currently being processed.
    #[sea_orm(num_value = 1)]
    InProgress,
    /// Has finished processing.
    #[sea_orm(num_value = 2)]
    Done,
    /// An error occurred during processing.
    #[sea_orm(num_value = 3)]
    Error,
}

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
pub enum TeamUpdateType {
    /// An update that changes the contracts on the team roster.
    #[default]
    #[sea_orm(num_value = 0)]
    Roster,
    /// An update that changes team settings.
    #[sea_orm(num_value = 1)]
    Setting,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
    #[sea_orm(has_many = "super::team_update_contract::Entity")]
    TeamUpdateContract,
    #[sea_orm(
        belongs_to = "super::transaction::Entity",
        from = "Column::TransactionId",
        to = "super::transaction::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Transaction,
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl Related<super::team_update_contract::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TeamUpdateContract.def()
    }
}

impl Related<super::transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Transaction.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn before_save(self, _insert: bool) -> Result<Self, DbErr> {
        roster_change_requires_transaction(&self)?;
        setting_change_requires_no_transaction(&self)?;

        Ok(self)
    }
}

fn roster_change_requires_transaction(model: &ActiveModel) -> Result<(), DbErr> {
    if model.update_type.as_ref() == &TeamUpdateType::Roster && model.transaction_id.is_not_set() {
        Err(DbErr::Custom(
            "A team update (roster change) requires a transaction id.".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn setting_change_requires_no_transaction(model: &ActiveModel) -> Result<(), DbErr> {
    if model.update_type.as_ref() == &TeamUpdateType::Setting && model.transaction_id.is_set() {
        Err(DbErr::Custom(
            "A team update (setting change) requires transaction id to be unset.".to_string(),
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamUpdateSettings {
    pub users: Vec<team_user::Model>,
}

impl TeamUpdateSettings {
    fn from_team_users(team_user_models: Vec<team_user::Model>) -> Self {
        Self {
            users: team_user_models,
        }
    }
}

/// Used for storing the state of a team.
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamUpdateSnapshot {
    pub settings: TeamUpdateSettings,
    pub contract_ids: Vec<i64>,
}

impl TeamUpdateSnapshot {
    async fn from_team_id<C>(team_id: i64, db: &C) -> Result<Self, DbErr>
    where
        C: ConnectionTrait + TransactionTrait,
    {
        // Fetch team users
        let team_users = team_user::Entity::find()
            .filter(team_user::Column::TeamId.eq(team_id))
            .all(db)
            .await?;

        let team_update_settings = TeamUpdateSettings::from_team_users(team_users);

        // Fetch contracts
        let contracts = contract::Entity::find()
            .select_only()
            .column(contract::Column::Id)
            .filter(
                contract::Column::TeamId
                    .eq(team_id)
                    .and(contract::Column::Status.eq(contract::ContractStatus::Active)),
            )
            .all(db)
            .await?;

        Ok(Self {
            settings: team_update_settings,
            contract_ids: contracts.into_iter().map(|contract| contract.id).collect(),
        })
    }

    fn as_bytes(&self) -> Result<Vec<u8>, Error> {
        // But what happens if the shape of the struct changes in the future?
        // I suppose you'd have to figure that out no matter how you store the data.
        let bytes_encoded = bincode::serialize(self)?;
        Ok(bytes_encoded)
    }

    fn from_bytes(bytes_encoded: &[u8]) -> Result<Self, Error> {
        let decoded: Option<Self> = bincode::deserialize(bytes_encoded)?;
        Ok(decoded.expect("Valid snapshot struct"))
    }
}
