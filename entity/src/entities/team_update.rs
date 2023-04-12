//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use crate::team_user;
use crate::team_user::LeagueRole;
use async_graphql::Enum;
use color_eyre::{eyre::Error, Result};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "team_update")]
pub struct Model {
    #[serde(skip_deserializing)]
    #[sea_orm(primary_key)]
    pub id: i64,
    pub update_type: TeamUpdateType,
    /// Data containing the update made to team settings or roster. Converted to/from TeamUpdateData.
    pub data: Vec<u8>,
    pub effective_date: Date,
    pub status: TeamUpdateStatus,
    pub team_id: i64,
    /// This is always present unless the update was a configuration change.
    pub transaction_id: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl Model {
    pub fn get_data(&self) -> Result<TeamUpdateData> {
        let data = TeamUpdateData::from_bytes(&self.data)?;
        Ok(data)
    }
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

/// Used for storing the roster or settings updates made to the team.
#[derive(Debug, Serialize, Deserialize)]
pub enum TeamUpdateData {
    /// The update to the team is a configuration change.
    Settings(TeamSettingsChange),
    /// The update to the team involves a roster change.
    Roster(Vec<ContractUpdate>),
}

impl TeamUpdateData {
    pub fn as_bytes(&self) -> Result<Vec<u8>, Error> {
        // But what happens if the shape of the struct changes in the future?
        // I suppose you'd have to figure that out no matter how you store the data.
        let bytes_encoded = postcard::to_allocvec(self)?;
        Ok(bytes_encoded)
    }

    pub fn from_bytes(bytes_encoded: &[u8]) -> Result<Self, Error> {
        let decoded: Self = postcard::from_bytes(bytes_encoded)?;
        Ok(decoded)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContractUpdate {
    pub contract_id: i64,
    pub update_type: ContractUpdateType,
}

/// Represents the different types of updates that can happen to a contract on a team.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContractUpdateType {
    /// A contract is dropped from a team.
    Drop,
    /// A contract is traded to another team.
    TradedAway,
    /// A contract is added to a team via trade.
    AddViaTrade,
    /// A contract is added to a team via auction.
    AddViaAuction,
    /// A contract is added via rookie draft selection.
    AddViaRookieDraft,
    /// A contract representing a rookie is activated.
    ActivateRookie,
    /// A contract is updated to IR status.
    ToIR,
    /// A contract is updated to no longer have IR status.
    FromIR,
    /// A contract is kept on the team for the Keeper Deadline.
    Keeper,
    /// A contract is advanced by one year.
    ContractAdvanced,
    /// A contract is lost to another team via Free Agency (in the Veteran Auction).
    LostViaFreeAgency,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamSettingsChange {
    pub users: Vec<TeamUpdateSettingUser>,
}

// impl TeamSettingsChange {
//     fn from_team_users(team_user_models: Vec<team_user::Model>) -> Self {
//         Self {
//             users: team_user_models
//                 .iter()
//                 .map(TeamUpdateSettingUser::from_team_user)
//                 .collect(),
//         }
//     }
// }

/// Like `team_user::Model`, but without the created_at/updated_at.
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamUpdateSettingUser {
    pub id: i64,
    pub league_role: LeagueRole,
    pub nickname: String,
    pub first_season_end_year: i16,
    pub final_season_end_year: Option<i16>,
    pub user_id: i64,
}

impl TeamUpdateSettingUser {
    pub fn from_team_user(team_user_model: &team_user::Model) -> Self {
        Self {
            id: team_user_model.id,
            league_role: team_user_model.league_role,
            nickname: team_user_model.nickname.clone(),
            first_season_end_year: team_user_model.first_season_end_year,
            final_season_end_year: team_user_model.final_season_end_year,
            user_id: team_user_model.user_id,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_update_data_encode_decode() -> Result<()> {
        let contract_update = ContractUpdate {
            contract_id: 1,
            update_type: ContractUpdateType::Keeper,
        };
        let team_update_data = TeamUpdateData::Roster(vec![contract_update.clone()]);

        let encoded_bytes = team_update_data.as_bytes()?;
        let decoded = TeamUpdateData::from_bytes(&encoded_bytes)?;

        match decoded {
            TeamUpdateData::Settings(_) => panic!("Settings not expected"),
            TeamUpdateData::Roster(contract_updates) => {
                assert_eq!(contract_update, contract_updates[0]);
            }
        };

        Ok(())
    }
}
