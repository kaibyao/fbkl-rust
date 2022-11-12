//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use async_graphql::Enum;
use color_eyre::eyre::{bail, Error};
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

/// A common misconception is that a player is owned/controlled by a team.
///
/// The truth is that when a player is signed to a team, what the team controls is the contract representing that player’s commitment to the team, as well as the team’s commitment to the player via the $-value being payed out to the player. Within a league, a player cannot have more than 1 non-expired contract at a time.
///
/// Additionally, a Contract is immutable. That is, we should never update an existing contract. Rather, we create a new/amended contract that points back to the previous contract. In this way, we can keep the history of changes made to a player's contract (which is really represented by the history chain of Contract models).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "contract")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub contract_year: i16,
    pub contract_type: ContractType,
    /// Represents an inactive/injured player on a team's active roster. A contract in IR status does not count towards a team’s cap space.
    pub is_ir: bool,
    /// The $-value paid out to the player via the team’s cap space.
    pub salary: i16,
    /// The year in which an NBA season ends.
    pub season_end_year: i16,
    /// Whether the contract is active, expired, or replaced by a newer contract. A newer contract will have this contract as its `previous_contract_id`.
    pub status: ContractStatus,
    pub league_id: i64,
    pub player_id: i64,
    pub previous_contract_id: Option<i64>,
    pub original_contract_id: Option<i64>,
    pub team_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// Whether the contract entering its next year is only incrementing the year of the contract while keeping the same contract type, is being signed to the original vs new team, or if it's being dropped.
#[derive(Debug, Default)]
pub enum ContractYearAdvancementType {
    /// Advancing the year of the current contract only. Applies to years: Veteran 1-2, Rookie 1-2, RookieExtention 4,
    #[default]
    AdvanceYearOnly,
    SignToOriginalTeam,
    SignToNewTeam,
    Drop,
}

impl Model {
    /// Creates the next year's contract from the current contract. This should be used in tandem with contract_queries::advance_contract, as we also need to update the current contract to point to the new one, plus handle various cases around RFAs/UFAs, and salaries.
    pub fn create_contract_year_advancement(
        &self,
        advancement_type: ContractYearAdvancementType,
        custom_bid_amount: Option<i16>,
    ) -> Result<ActiveModel, Error> {
        let mut new_contract = ActiveModel {
            id: NotSet,
            contract_year: Set(self.contract_year),
            contract_type: Set(self.contract_type),
            is_ir: Set(self.is_ir),
            salary: Set(self.salary),
            season_end_year: Set(self.season_end_year + 1),
            status: Set(self.status),
            league_id: Set(self.league_id),
            player_id: Set(self.player_id),
            previous_contract_id: Set(Some(self.id)),
            original_contract_id: Set(self.original_contract_id),
            team_id: Set(self.team_id),
            created_at: NotSet,
            updated_at: NotSet,
        };

        match self.contract_type {
            ContractType::RookieDevelopment | ContractType::RookieDevelopmentInternational => match self.contract_year {
                1 => {
                    new_contract.contract_year = Set(2);
                },
                2 => {
                    new_contract.contract_year = Set(3);
                },
                3 => {
                    new_contract.contract_year = Set(1);
                    new_contract.contract_type = Set(ContractType::Rookie);
                },
                _ => {
                    bail!("Invalid year for contract type: ({:?}, {})", self.contract_type, self.contract_year);
                }
            },
            ContractType::Rookie => match self.contract_year {
                    1 => {
                        new_contract.contract_year = Set(2);
                    },
                    2 => {
                        new_contract.contract_year = Set(3);
                    },
                    3 => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::RestrictedFreeAgent);
                    },
                    _ => {
                        bail!("Invalid year for contract type: ({:?}, {})", self.contract_type, self.contract_year);
                    }
                },
            ContractType::RestrictedFreeAgent =>
                match advancement_type {
                    ContractYearAdvancementType::AdvanceYearOnly => bail!("Advancing an RFA contract requires the player to be either dropped or signed to a team."),
                    ContractYearAdvancementType::SignToOriginalTeam => {
                        new_contract.contract_year = Set(4);
                        new_contract.contract_type = Set(ContractType::RookieExtension);
                    },
                    ContractYearAdvancementType::SignToNewTeam => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::Veteran);
                    },
                    ContractYearAdvancementType::Drop => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::FreeAgent);
                    },
                }
            ,
            ContractType::RookieExtension =>
                match self.contract_year {
                    4 => {
                        new_contract.contract_year = Set(5);
                    },
                    5 => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::UnrestrictedFreeAgentOriginalTeam);
                    },
                    _ => {
                        bail!("Invalid year for contract type: ({:?}, {})", self.contract_type, self.contract_year);
                    }
                }
            ,
            ContractType::Veteran => {
                match self.contract_year {
                    1 => {
                        new_contract.contract_year = Set(2);
                    },
                    2 => {
                        new_contract.contract_year = Set(3);
                    },
                    3 => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::UnrestrictedFreeAgentVeteran);
                    },
                    _ => {
                        bail!("Invalid year for contract type: ({:?}, {})", self.contract_type, self.contract_year);
                    }
                }
            },
            ContractType::FreeAgent => {
                // The idea being that advancing a FA contract means the player being signed to a team..
                new_contract.contract_year = Set(1);
                new_contract.contract_type = Set(ContractType::Veteran);
            },
            ContractType::UnrestrictedFreeAgentOriginalTeam => {
                match advancement_type {
                    ContractYearAdvancementType::AdvanceYearOnly => bail!("Advancing a UFA contract requires the player to be either dropped or signed to a team."),
                    ContractYearAdvancementType::Drop => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::FreeAgent);
                    },
                    _ => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::Veteran);
                    },
                }
            },
            ContractType::UnrestrictedFreeAgentVeteran => {
                match advancement_type {
                    ContractYearAdvancementType::AdvanceYearOnly => bail!("Advancing a UFA contract requires the player to be either dropped or signed to a team."),
                    ContractYearAdvancementType::Drop => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::FreeAgent);
                    },
                    _ => {
                        new_contract.contract_year = Set(1);
                        new_contract.contract_type = Set(ContractType::Veteran);
                    },
                }
            },
        }

        new_contract.salary =
            Set(self.calculate_yearly_salary_increase(&advancement_type, custom_bid_amount)?);

        Ok(new_contract)
    }

    /// Not doing validation in this function because it's already done in `create_contract_year_advancement`.
    fn calculate_yearly_salary_increase(
        &self,
        advancement_type: &ContractYearAdvancementType,
        custom_bid_amount: Option<i16>,
    ) -> Result<i16, Error> {
        match self.contract_type {
            ContractType::RookieDevelopment | ContractType::RookieDevelopmentInternational => {
                Ok(self.salary)
            }
            ContractType::Rookie => Ok(Model::get_salary_increased_by_20_percent(self.salary)),
            ContractType::RestrictedFreeAgent => match advancement_type {
                ContractYearAdvancementType::SignToOriginalTeam => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Signing a restricted free agent to a team requires a custom/winning bid amount.")
                    };
                    Ok(Model::get_salary_discounted_by_10_percent(bid_amount))
                }
                ContractYearAdvancementType::SignToNewTeam => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Signing a restricted free agent to a team requires a custom/winning bid amount.")
                    };

                    Ok(bid_amount)
                }
                _ => Ok(self.salary),
            },
            ContractType::RookieExtension => match self.contract_year {
                4 => Ok(Model::get_salary_increased_by_20_percent(self.salary)),
                _ => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Moving to a UFA contract requires a custom starting salary.")
                    };
                    Ok(bid_amount)
                }
            },
            ContractType::UnrestrictedFreeAgentOriginalTeam => match advancement_type {
                ContractYearAdvancementType::AdvanceYearOnly => bail!(
                    "Impossible combination: adding a yearly salary increase to a UFA20 contract."
                ),
                ContractYearAdvancementType::SignToOriginalTeam => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Signing an unrestricted free agent to their original team requires a custom/winning bid amount.")
                    };
                    Ok(Model::get_salary_discounted_by_20_percent(bid_amount))
                }
                ContractYearAdvancementType::SignToNewTeam => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Signing an unrestricted free agent to a new team requires a custom/winning bid amount.")
                    };
                    Ok(bid_amount)
                }
                ContractYearAdvancementType::Drop => Ok(1),
            },
            ContractType::Veteran => match self.contract_year {
                1 | 2 => Ok(Model::get_salary_increased_by_20_percent(self.salary)),
                _ => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Moving to a UFA contract requires a custom starting salary.")
                    };
                    Ok(bid_amount)
                }
            },
            ContractType::UnrestrictedFreeAgentVeteran => match advancement_type {
                ContractYearAdvancementType::AdvanceYearOnly => bail!(
                    "Impossible combination: adding a yearly salary increase to a UFA10 contract."
                ),
                ContractYearAdvancementType::SignToOriginalTeam => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Signing an unrestricted free agent to their original team requires a custom/winning bid amount.")
                    };
                    Ok(Model::get_salary_discounted_by_10_percent(bid_amount))
                }
                ContractYearAdvancementType::SignToNewTeam => {
                    let Some(bid_amount) = custom_bid_amount else {
                        bail!("Signing an unrestricted free agent to a new team requires a custom/winning bid amount.")
                    };
                    Ok(bid_amount)
                }
                ContractYearAdvancementType::Drop => Ok(1),
            },
            // Increasing the yearly salary of an FA = signing to a team
            ContractType::FreeAgent => {
                let Some(bid_amount) = custom_bid_amount else {
                    bail!("Signing an unrestricted free agent to a new team requires a custom/winning bid amount.")
                };
                Ok(bid_amount)
            }
        }
    }

    fn get_salary_increased_by_20_percent(salary: i16) -> i16 {
        let increased_salary = f32::from(salary) * 1.2;
        increased_salary.ceil() as i16
    }

    fn get_salary_discounted_by_10_percent(salary: i16) -> i16 {
        let discount_amount_rounded_up = (f32::from(salary) * 0.1).ceil();
        salary - (discount_amount_rounded_up as i16)
    }

    fn get_salary_discounted_by_20_percent(salary: i16) -> i16 {
        let discount_amount_rounded_up = (f32::from(salary) * 0.2).ceil();
        salary - (discount_amount_rounded_up as i16)
    }
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
    RookieDevelopment, // --------------- RD 1-3
    /// A player that has been drafted but is playing overseas can be moved from a RD contract to an RDI contract.
    #[sea_orm(num_value = 1)]
    RookieDevelopmentInternational, // -- RDI 1-3
    /// When a player that was previously on an RD contract is activated, they are converted to a Rookie contract, which lasts up to 3 years.
    #[sea_orm(num_value = 2)]
    Rookie, // -------------------------- R 1-3
    /// A player that is on their 3rd rookie season (Rookie) is converted to an RFA contract for the duration of the following preseason. They are then re-signed (RookieExtension) at a 10% discount, signed to a different team (Veteran), or dropped (expired status).
    #[sea_orm(num_value = 3)]
    RestrictedFreeAgent, // ------------- RFA - 10%
    /// After a rookie has been active for 3 years, they become a restricted free agent. If re-signed to their original team (original team being the team that controlled their rookie contract), they are converted to a rookie extension, which lasts up to 2 years.
    #[sea_orm(num_value = 4)]
    RookieExtension, // ----------------- R 4-5
    /// A player that is on their 5th rookie season (RookieExtension) is converted to a UFA-20 for the duration of the following preseason. They are then re-signed (Veteran) at a 20% discount, signed to a different team (Veteran, no discount), or dropped (expired status).
    #[sea_orm(num_value = 5)]
    UnrestrictedFreeAgentOriginalTeam, // UFA - 20%
    /// Signing a free agent to a team puts them on a V contract, which lasts up to 3 years.
    #[default]
    #[sea_orm(num_value = 6)]
    Veteran, // ------------------------- V 1-3
    /// A player that is on their 3rd veteran season is converted to a UFA-10 for the duration of the following preseason. They are then re-signed (Veteran) at a 10% discount, signed to a different team (Veteran, no discount), or dropped (expired status).
    #[sea_orm(num_value = 7)]
    UnrestrictedFreeAgentVeteran, // ---- UFA - 10%
    /// Signing on a new player via auction creates a FA contract, and dropping a player from a team for any reason moves them to free agency. A free agent can be signed onto any team starting from the beginning of the week after they are dropped.
    #[sea_orm(num_value = 8)]
    FreeAgent, // This is needed when resigning previously-dropped players, as we need to know their previous contract's value.
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
    /// Represents a contract that has been replaced by a newer contract. The current contract should be referenced by the newer contract's previous_contract_id. A replaced contract is not counted towards a team’s cap space.
    #[sea_orm(num_value = 1)]
    Replaced,
    /// Represents a contract that is no longer valid and should not be used as a previous_contract_id for any other contract. This happens at the start of each season (for the previous season's contracts), as well as when an RFA/UFA/RD/RDI contract is dropped.
    #[sea_orm(num_value = 2)]
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
    #[sea_orm(has_many = "super::team_update_contract::Entity")]
    TeamUpdateContract,
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

impl Related<super::team_update_contract::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TeamUpdateContract.def()
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
