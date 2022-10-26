//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A common misconception is that a player is owned/controlled by a team.
///
/// The truth is that when a player is signed to a team, what the team controls is the contract representing that player’s commitment to the team, as well as the team’s commitment to the player via the $-value being payed out to the player. Within a league, a player cannot have more than 1 non-expired contract at a time.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "contract")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub contract_year: i16,
    pub contract_type: ContractType,
    /// The $-value paid out to the player via the team’s cap space.
    pub salary: i16,
    pub season_end_year: i16,
    pub status: ContractStatus,
    pub league_id: i64,
    pub player_id: i64,
    pub previous_contract_id: Option<i64>,
    pub original_contract_id: Option<i64>,
    pub team_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// Represents the different types of contract to which a player can be signed. When a player is signed to a team, their contract must be of one of these types.
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
pub enum ContractType {
    /// When a drafted player has not yet been activated, they are considered to be on a RD contract. An RD contract can last up to 3 years, though activating an RD player on their 4th year on a team converts them to their second year as a Rookie (R2 instead of R1).
    #[sea_orm(num_value = 0)]
    RookieDevelopment, // --------------- RD1-3
    /// When a player that was previously on an RD contract is activated, they are converted to a Rookie contract, which lasts up to 3 years.
    #[sea_orm(num_value = 1)]
    Rookie, // -------------------------- R1-3
    /// After a rookie has been active for 3 years, they become a restricted free agent. If re-signed to their original team (original team being the team that controlled their rookie contract), they are converted to a rookie extension, which lasts up to 2 years.
    #[sea_orm(num_value = 2)]
    RookieExtension, // ----------------- R4-5
    /// Signing a free agent to a team puts them on a V contract, which lasts up to 3 years.
    #[default]
    #[sea_orm(num_value = 3)]
    Veteran, // ------------------------- V1-3
    /// Dropping a player from a team for any reason moves them to free agency. A free agent can be signed onto any team starting from the beginning of the week after they are dropped.
    #[sea_orm(num_value = 4)]
    FreeAgent, // This is needed when resigning previously-dropped players, as we need to know their previous contract's value.
    /// A player that is on their 3rd rookie season (Rookie) is converted to an RFA contract for the duration of the following preseason. They are then re-signed (RookieExtension) at a 10% discount, signed to a different team (Veteran), or dropped (expired status).
    #[sea_orm(num_value = 5)]
    RestrictedFreeAgent, // ------------- RFA - 10%
    /// A player that is on their 5th rookie season (RookieExtension) is converted to a UFA-20 for the duration of the following preseason. They are then re-signed (Veteran) at a 20% discount, signed to a different team (Veteran, no discount), or dropped (expired status).
    #[sea_orm(num_value = 6)]
    UnrestrictedFreeAgentOriginalTeam, // UFA - 20%
    /// A player that is on their 3rd veteran season is converted to a UFA-10 for the duration of the following preseason. They are then re-signed (Veteran) at a 10% discount, signed to a different team (Veteran, no discount), or dropped (expired status).
    #[sea_orm(num_value = 7)]
    UnrestrictedFreeAgentVeteran, // ---- UFA - 10%
    /// A player that has been drafted but is playing overseas can be moved from a RD contract to an RDI contract.
    #[sea_orm(num_value = 8)]
    RookieDevelopmentInternational, // -- RDI
}

/// Represents whether the contract is currently active for a player.
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
pub enum ContractStatus {
    /// Represents an active player on a team. An active contract’s $-value takes up a team’s cap space.
    #[default]
    #[sea_orm(num_value = 0)]
    Active,
    /// Represents an inactive/injured player on a team's active roster. A contract in IR status does not count towards a team’s cap space.
    #[sea_orm(num_value = 1)]
    IR,
    /// Represents a contract that has been replaced by a newer contract. The current contract should be referenced by the newer contract's previous_contract_id. A replaced contract is not counted towards a team’s cap space.
    #[sea_orm(num_value = 3)]
    Replaced,
    /// Represents a contract that is no longer valid and should not be used as a previous_contract_id for any other contract. This happens at the start of each season (for the previous season's contracts), as well as when an RFA/UFA/RD/RDI contract is dropped.
    #[sea_orm(num_value = 4)]
    Expired,
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
        from = "Column::OriginalContractId",
        to = "Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    OriginalContract,
    #[sea_orm(
        belongs_to = "super::player::Entity",
        from = "Column::PlayerId",
        to = "super::player::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Player,
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::PreviousContractId",
        to = "Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    PreviousContract,
    #[sea_orm(
        belongs_to = "super::team::Entity",
        from = "Column::TeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<super::player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

pub struct OriginalContract;
impl Linked for OriginalContract {
    type FromEntity = Entity;
    type ToEntity = Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::OriginalContract.def()]
    }
}

pub struct PreviousContract;
impl Linked for PreviousContract {
    type FromEntity = Entity;
    type ToEntity = Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::PreviousContract.def()]
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn before_save(self, insert: bool) -> Result<Self, DbErr> {
        non_original_contract_requires_previous_contract(&self)?;
        original_contract_requires_unset_previous_contract(&self)?;
        validate_contract_years_by_type(&self)?;

        if !insert {
            update_requires_original_contract(&self)?;
        }

        Ok(self)
    }
}

fn non_original_contract_requires_previous_contract(model: &ActiveModel) -> Result<(), DbErr> {
    if model.previous_contract_id.is_not_set()
        && model.original_contract_id.is_set()
        && model.original_contract_id.as_ref().as_ref().unwrap() != model.id.as_ref()
    {
        Err(DbErr::Custom(format!("This contract (id={}, original_contract_id={:?}) is missing a reference to the previous contract for this player.", model.id.as_ref(), model.original_contract_id.as_ref())))
    } else {
        Ok(())
    }
}

fn original_contract_requires_unset_previous_contract(model: &ActiveModel) -> Result<(), DbErr> {
    if model.previous_contract_id.is_set()
        && model.original_contract_id.is_set()
        && model.original_contract_id.as_ref().as_ref().unwrap() == model.id.as_ref()
    {
        Err(DbErr::Custom(format!("This contract (id={}, original_contract_id={:?}, previous_contract_id={:?}) is supposedly the original (id and original id are matching), yet a previous contract id is referenced.", model.id.as_ref(), model.original_contract_id.as_ref(), model.previous_contract_id.as_ref())))
    } else {
        Ok(())
    }
}

fn update_requires_original_contract(model: &ActiveModel) -> Result<(), DbErr> {
    if model.original_contract_id.is_not_set() {
        Err(DbErr::Custom(format!(
            "This contract (id={}) requires original_contract_id to be set before it can be saved.",
            model.id.as_ref()
        )))
    } else {
        Ok(())
    }
}

static VALID_CONTRACT_TYPE_YEARS: &[&(&ContractType, &[i16])] = &[
    &(&ContractType::RookieDevelopment, &[1, 2, 3]),
    &(&ContractType::RookieDevelopmentInternational, &[1, 2, 3]),
    &(&ContractType::Rookie, &[1, 2, 3]),
    &(&ContractType::RookieExtension, &[4, 5]),
    &(&ContractType::Veteran, &[1, 2, 3]),
];

fn validate_contract_years_by_type(model: &ActiveModel) -> Result<(), DbErr> {
    match VALID_CONTRACT_TYPE_YEARS
        .iter()
        .find(|(contract_type, _years_allowed)| contract_type == &model.contract_type.as_ref())
    {
        None => Ok(()),
        Some((_contract_type, valid_years_for_contract_type)) => {
            match valid_years_for_contract_type.contains(model.contract_year.as_ref()) {
                true => Ok(()),
                false => Err(DbErr::Custom(format!(
                    "contract_year value ({:?}) for contract type ({:?}) not allowed.",
                    model.contract_year.as_ref(),
                    model.contract_type.as_ref()
                ))),
            }
        }
    }
}