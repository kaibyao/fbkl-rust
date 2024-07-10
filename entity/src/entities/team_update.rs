//! A Team Update contains information about a change that was made to a team within a league. Anything that changes a team's settings, roster, or draft picks is stored as a Team Update. This allows us to look back at a team's history of changes.

use crate::team_user;
use crate::team_user::LeagueRole;
use async_trait::async_trait;
use color_eyre::Result;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A Team Update contains information about a change that was made to a team within a league. Anything that changes a team's settings or assets owned is stored as a Team Update. This allows us to look back at a team's history of changes.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "team_update")]
pub struct Model {
    #[serde(skip_deserializing)]
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Data containing the update made to team settings or roster. Converted to/from TeamUpdateData.
    pub data: serde_json::Value,
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
        let data: TeamUpdateData = serde_json::from_value(self.data.clone())?;
        Ok(data)
    }
}

#[derive(
    Debug, Default, Clone, Copy, Eq, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum TeamUpdateStatus {
    /// Has not been processed yet.
    #[default]
    #[sea_orm(string_value = "Pending")]
    Pending,
    /// Is currently being processed.
    #[sea_orm(string_value = "InProgress")]
    InProgress,
    /// Has finished processing.
    #[sea_orm(string_value = "Done")]
    Done,
    /// An error occurred during processing.
    #[sea_orm(string_value = "Error")]
    Error,
}

/// Used for storing the roster or settings updates made to the team.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TeamUpdateData {
    /// The update to the team involves changes to its owned assets.
    Assets(TeamUpdateAssetSummary),
    /// The update to the team is a configuration change.
    Settings(TeamSettingsChange),
}

impl TeamUpdateData {
    /// Generates a new data struct from given assets.
    pub fn from_assets(
        all_contract_ids: Vec<i64>,
        changed_assets: Vec<TeamUpdateAsset>,
        new_salary: i16,
        new_salary_cap: i16,
        previous_salary: i16,
        previous_salary_cap: i16,
    ) -> Self {
        TeamUpdateData::Assets(TeamUpdateAssetSummary {
            all_contract_ids,
            changed_assets,
            new_salary,
            new_salary_cap,
            previous_salary,
            previous_salary_cap,
        })
    }

    /// Consumes & converts the team update data into a json value, to be stored in the database.
    pub fn to_json(self) -> Result<serde_json::Value> {
        let self_as_json: serde_json::Value = serde_json::value::to_value(self)?;
        Ok(self_as_json)
    }

    pub fn from_json(data_as_json: serde_json::Value) -> Result<Self> {
        let data: Self = serde_json::value::from_value(data_as_json)?;
        Ok(data)
    }
}

/// Stores asset changes (contracts, draft picks) as well as salary changes to a team roster.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TeamUpdateAssetSummary {
    /// Contract IDs that map to ALL contracts owned by the team.
    pub all_contract_ids: Vec<i64>,
    pub changed_assets: Vec<TeamUpdateAsset>,
    pub new_salary: i16,
    pub new_salary_cap: i16,
    pub previous_salary: i16,
    pub previous_salary_cap: i16,
}

/// Stores information about changes made to a team's assets.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TeamUpdateAsset {
    /// The update to the team involves a roster change.
    Contracts(Vec<ContractUpdate>),
    /// The update to the team involves changes to its owned draft pick(s).
    DraftPicks(Vec<DraftPickUpdate>),
}

/// Stores data for an update to a team's contract.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContractUpdate {
    pub contract_id: i64,
    pub update_type: ContractUpdateType,
    pub player_name_at_time: String,
    pub player_team_abbr_at_time: String,
    pub player_team_name_at_time: String,
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
    /// A contract representing a rookie development is activated.
    ActivateRookie,
    /// A contract is updated to IR status.
    ToIR,
    /// A contract is updated to no longer have IR status.
    FromIR,
    /// An RD contract is updated to RDI status.
    ToRdi,
    /// An RDI contract is updated to RD status.
    FromRdi,
    /// A contract is kept on the team for the Keeper Deadline.
    Keeper,
    /// A contract is advanced by one year.
    ContractAdvanced,
    /// A contract is lost to another team via Free Agency (in the Veteran Auction).
    LostViaFreeAgency,
}

/// Stores data for an update to a team's draft pick.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DraftPickUpdate {
    pub draft_pick_id: i64,
    pub update_type: DraftPickUpdateType,
    pub added_draft_pick_option_id: Option<i64>,
}

/// Represents the different types of updates that can happen to a team's draft pick(s)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum DraftPickUpdateType {
    /// A draft pick is traded to another team.
    TradedAway,
    /// A draft pick is added to the team via a completed trade.
    AddViaTrade,
    /// A draft pick option has been added to the draft pick.
    DraftPickOptionAdded,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TeamSettingsChange {
    pub users: Vec<TeamUpdateSettingUser>,
}

/// Like `team_user::Model`, but without the created_at/updated_at.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TeamUpdateSettingUser {
    pub id: i64,
    pub league_role: LeagueRole,
    pub nickname: String,
    pub first_end_of_season_year: i16,
    pub final_end_of_season_year: Option<i16>,
    pub user_id: i64,
}

impl TeamUpdateSettingUser {
    pub fn from_team_user(team_user_model: &team_user::Model) -> Self {
        Self {
            id: team_user_model.id,
            league_role: team_user_model.league_role,
            nickname: team_user_model.nickname.clone(),
            first_end_of_season_year: team_user_model.first_end_of_season_year,
            final_end_of_season_year: team_user_model.final_end_of_season_year,
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

impl Related<super::deadline::Entity> for Entity {
    fn to() -> RelationDef {
        super::transaction::Relation::Deadline.def()
    }

    fn via() -> Option<RelationDef> {
        Some(Relation::Transaction.def())
    }
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

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        roster_change_requires_transaction(&self)?;
        setting_change_requires_no_transaction(&self)?;

        Ok(self)
    }
}

fn roster_change_requires_transaction(model: &ActiveModel) -> Result<(), DbErr> {
    let decoded_data = TeamUpdateData::from_json(model.data.as_ref().clone())
        .map_err(|err| DbErr::Custom(err.to_string()))?;
    let is_assets_update = matches!(decoded_data, TeamUpdateData::Assets(_));

    if is_assets_update && model.transaction_id.is_not_set() {
        Err(DbErr::Custom(
            "A team update (roster change) requires a transaction id.".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn setting_change_requires_no_transaction(model: &ActiveModel) -> Result<(), DbErr> {
    let decoded_data = TeamUpdateData::from_json(model.data.as_ref().clone())
        .map_err(|err| DbErr::Custom(err.to_string()))?;
    let is_settings_update = matches!(decoded_data, TeamUpdateData::Settings(_));

    if is_settings_update && model.transaction_id.is_set() {
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
            player_name_at_time: "Enes Kanter".to_string(),
            player_team_abbr_at_time: "BOS".to_string(),
            player_team_name_at_time: "Boston Celtics".to_string(),
        };
        let draft_pick_update = DraftPickUpdate {
            draft_pick_id: 1,
            update_type: DraftPickUpdateType::AddViaTrade,
            added_draft_pick_option_id: None,
        };
        let team_update_assets = vec![
            TeamUpdateAsset::DraftPicks(vec![draft_pick_update]),
            TeamUpdateAsset::Contracts(vec![contract_update]),
        ];
        let team_update_data =
            TeamUpdateData::from_assets(vec![1], team_update_assets, 98, 100, 200, 189);

        let encoded = team_update_data.clone().to_json()?;
        let decoded = TeamUpdateData::from_json(encoded)?;

        assert_eq!(decoded, team_update_data);

        Ok(())
    }
}
