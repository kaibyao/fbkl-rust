//! SeaORM Entity. Generated by sea-orm-codegen 0.9.2

use std::fmt::Debug;

use async_graphql::Enum;
use async_trait::async_trait;
use color_eyre::{
    eyre::{bail, eyre, Error},
    Result,
};
use sea_orm::{entity::prelude::*, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    auction, league, league_player, player, rookie_draft_selection, team, trade_asset, transaction,
};

use super::{
    annual_contract_advancement::create_advancement_for_contract,
    drop_contract::create_dropped_contract, expire_contract::expire_contract,
    free_agent_extension::sign_rfa_or_ufa_contract_to_team,
    rookie_activation::create_rookie_contract_from_rd,
    rookie_draft::new_contract_from_rookie_draft, trade_contract::trade_contract_to_team,
    veteran_auction_contract::new_contract_for_veteran_auction,
    veteran_contract_signing::sign_veteran_contract,
};

/// A common misconception is that a player is owned/controlled by a team.
///
/// The truth is that when a player is signed to a team, what the team controls is the contract representing that player’s commitment to the team, as well as the team’s commitment to the player via the $-value being payed out to the player. Within a league, a player cannot have more than 1 non-expired contract at a time.
///
/// Additionally, a Contract is immutable. That is, we should never update an existing contract. Rather, we create a new/amended contract that points back to the previous contract. In this way, we can keep the history of changes made to a player's contract (which is represented by the history chain of Contract models).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "contract")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub contract_year: i16,
    /// RD | RDI | R1-3 | R4-5 | V | RFA10 | UFA10 | UFA20 | FA
    pub contract_type: ContractType,
    /// Represents an inactive/injured player on a team's active roster. A contract in IR status does not count towards a team’s cap space.
    pub is_ir: bool,
    /// The $-value paid out to the player via the team’s cap space.
    pub salary: i16,
    /// The year in which an NBA season ends.
    pub end_of_season_year: i16,
    /// Whether the contract is active, expired, or replaced by a newer contract. A newer contract will have this contract as its `previous_contract_id`.
    pub status: ContractStatus,
    pub league_id: i64,
    pub league_player_id: Option<i64>,
    pub player_id: Option<i64>,
    /// All non-original contracts have a previous_contract_id. All original contracts have this set to `None`.
    pub previous_contract_id: Option<i64>,
    /// The root-level contract that started the contract chain. The root-level contract has this field set to its `id`.
    pub original_contract_id: Option<i64>,
    /// Points to a league's team id. The only time this field is `None` is during an auction, where a player has yet to be on a team for the season.
    pub team_id: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl Model {
    /// Creates a new contract that converts the RD(I) contract to a standard rookie contract. Note that this doesn't do anything to insert the new contract or update the original.
    pub fn activate_rookie(&self) -> Result<ActiveModel, Error> {
        create_rookie_contract_from_rd(self)
    }

    /// Creates the next year's contract from the current contract. This should be used in tandem with contract_queries::advance_contract, as we also need to update the current contract to point to the new one, plus handle various cases around RFAs/UFAs, and salaries.
    pub fn create_annual_contract_advancement(&self) -> Result<ActiveModel, Error> {
        create_advancement_for_contract(self)
    }

    /// Creates a new contract that drops the player from the team. Note that this doesn't do anything to insert the new contract or update the original.
    pub fn create_dropped_contract_before_preseason_keeper_deadline(
        &self,
    ) -> Result<ActiveModel, Error> {
        create_dropped_contract(self, true)
    }

    /// Creates a new contract that drops the player from the team. Note that this doesn't do anything to insert the new contract or update the original.
    pub fn create_dropped_contract_after_preseason_keeper_deadline(
        &self,
    ) -> Result<ActiveModel, Error> {
        create_dropped_contract(self, false)
    }

    /// Creates a new contract (not yet inserted) from this one, where the contract is expired.
    pub fn create_expired_contract(&self) -> Result<ActiveModel, Error> {
        expire_contract(self)
    }

    /// Retrieves the player model related to this contract.
    #[instrument]
    pub async fn get_player<C>(&self, db: &C) -> Result<RelatedPlayer>
    where
        C: ConnectionTrait + Debug,
    {
        match self.find_related(player::Entity).one(db).await? {
            Some(player_model) => Ok(RelatedPlayer::Player(player_model)),
            None => match self.find_related(league_player::Entity).one(db).await? {
                None => bail!(
                    "Could not find related player or league_player for contract (id = {})",
                    self.id
                ),
                Some(league_player_model) => Ok(RelatedPlayer::LeaguePlayer(league_player_model)),
            },
        }
    }

    /// Retrieves the team model related to this contract.
    #[instrument]
    pub async fn get_team<C>(&self, db: &C) -> Result<Option<team::Model>>
    where
        C: ConnectionTrait + Debug,
    {
        let maybe_team_model = self.find_related(team::Entity).one(db).await?;
        Ok(maybe_team_model)
    }

    /// Retrieves the latest contract in the contract history chain.
    pub async fn get_latest_in_chain<C>(&self, db: &C) -> Result<Model>
    where
        C: ConnectionTrait + Debug,
    {
        let mut all_contracts_in_chain = Entity::find()
            .filter(Column::OriginalContractId.eq(self.original_contract_id))
            .all(db)
            .await?;
        all_contracts_in_chain.sort_by(|a, b| a.id.cmp(&b.id));
        all_contracts_in_chain.pop().ok_or_else(|| {
            let contract_ids_in_chain: Vec<String> = all_contracts_in_chain
                .iter()
                .map(|contract| contract.id.to_string())
                .collect();
            eyre!(
                "Could not retrieve last contract in contract chain: [{}], called on contract (id = {})",
                contract_ids_in_chain.join(", "),
                self.id
            )
        })
    }

    /// Checks whether this contract is the most recent in the contract history chain.
    pub async fn is_latest_in_chain<C>(&self, db: &C) -> Result<bool>
    where
        C: ConnectionTrait + Debug,
    {
        let last_contract_in_history_chain = self.get_latest_in_chain(db).await?;
        Ok(last_contract_in_history_chain.id == self.id)
    }

    // Returns the contract (now in IR) to be added to the contract chain.
    pub fn move_to_ir(&self) -> ActiveModel {
        let mut updated_contract: ActiveModel = self.clone().into();
        updated_contract.id = ActiveValue::NotSet;
        updated_contract.is_ir = ActiveValue::Set(true);
        updated_contract.previous_contract_id = ActiveValue::Set(Some(self.id));
        updated_contract
    }

    pub fn new_contract_for_veteran_auction(
        league_id: i64,
        end_of_season_year: i16,
        player_id: i64,
    ) -> ActiveModel {
        new_contract_for_veteran_auction(league_id, end_of_season_year, player_id)
    }

    pub fn new_contract_from_rookie_draft(
        league_id: i64,
        end_of_season_year: i16,
        team_id: i64,
        salary: i16,
        player_id: i64,
        is_league_player: bool,
    ) -> ActiveModel {
        new_contract_from_rookie_draft(
            league_id,
            end_of_season_year,
            team_id,
            salary,
            player_id,
            is_league_player,
        )
    }

    /// Creates a new Veteran or Rookie Extension contract from the current contract as a result of a team winning the contract during the Preseason Veteran Auction. Note that this doesn't do anything to insert the new contract or update the original.
    pub fn sign_rfa_or_ufa_contract_to_team(
        &self,
        team_id: i64,
        winning_bid_amount: i16,
    ) -> Result<ActiveModel, Error> {
        sign_rfa_or_ufa_contract_to_team(self, team_id, winning_bid_amount)
    }

    /// Creates a new Veteran contract from the current contract as a result of a team winning the contract in an auction (either Veteran or in-season FA). Note that this doesn't do anything to insert the new contract or update the original.
    pub fn sign_veteran_contract_to_team(
        &self,
        team_id: i64,
        salary: i16,
    ) -> Result<ActiveModel, Error> {
        sign_veteran_contract(self, team_id, salary)
    }

    /// Creates a new contract in the history chain to denote that the contract has been traded to a new team. Note that this doesn't do anything to insert the new contract or update the original.
    pub fn trade_contract_to_team(&self, new_team_id: i64) -> ActiveModel {
        trade_contract_to_team(self, new_team_id)
    }
}

/// An abstraction that allows the contract model to return its related player, regardless of whether it's a player model or league-specific player model.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum RelatedPlayer {
    LeaguePlayer(league_player::Model),
    Player(player::Model),
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
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum ContractType {
    /// When a drafted player has not yet been activated, they are considered to be on a RD contract. An RD contract can last up to 3 years, though activating an RD player on their 4th year on a team converts them to their second year as a Rookie (R2 instead of R1).
    #[sea_orm(string_value = "RD")]
    RookieDevelopment, // --------------- RD 1-3
    /// A player that has been drafted but is playing overseas can be moved from a RD contract to an RDI contract.
    #[sea_orm(string_value = "RDI")]
    RookieDevelopmentInternational, // -- RDI 1-3
    /// When a player that was previously on an RD contract is activated, they are converted to a Rookie contract, which lasts up to 3 years.
    #[sea_orm(string_value = "Rookie")]
    Rookie, // -------------------------- R 1-3
    /// A player that is on their 3rd rookie season (Rookie) is converted to an RFA contract for the duration of the following preseason. They are then re-signed (RookieExtension) at a 10% discount, signed to a different team (Veteran), or dropped (expired status).
    #[sea_orm(string_value = "RFA")]
    RestrictedFreeAgent, // ------------- RFA - 10%
    /// After a rookie has been active for 3 years, they become a restricted free agent. If re-signed to their original team (original team being the team that controlled their rookie contract), they are converted to a rookie extension, which lasts up to 2 years.
    #[sea_orm(string_value = "RookieExtension")]
    RookieExtension, // ----------------- R 4-5
    /// A player that is on their 5th rookie season (RookieExtension) is converted to a UFA-20 for the duration of the following preseason. They are then re-signed (Veteran) at a 20% discount, signed to a different team (Veteran, no discount), or dropped (expired status).
    #[sea_orm(string_value = "UFA-OriginalTeam")]
    UnrestrictedFreeAgentOriginalTeam, // UFA - 20%
    /// Signing a free agent to a team puts them on a V contract, which lasts up to 3 years.
    #[default]
    #[sea_orm(string_value = "Veteran")]
    Veteran, // ------------------------- V 1-3
    /// A player that is on their 3rd veteran season is converted to a UFA-10 for the duration of the following preseason. They are then re-signed (Veteran) at a 10% discount, signed to a different team (Veteran, no discount), or dropped (expired status).
    #[sea_orm(string_value = "UFA-FreeAgent")]
    UnrestrictedFreeAgentVeteran, // ---- UFA - 10%
    /// Signing on a new player via auction creates a Veteran contract, and dropping a player from a team for any reason moves them to free agency. A free agent can be signed onto any team starting from the beginning of the week after they are dropped.
    #[sea_orm(string_value = "FreeAgent")]
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
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum ContractStatus {
    /// Represents an active player on a team or in free agency. An active contract’s $-value takes up a team’s cap space.
    #[default]
    #[sea_orm(string_value = "Active")]
    Active,
    /// Represents a contract that has been replaced by a newer contract. The current contract should be referenced by the newer contract's previous_contract_id. A replaced contract is not counted towards a team’s cap space.
    #[sea_orm(string_value = "Replaced")]
    Replaced,
    /// Represents a contract that is no longer valid and should not be used as a previous_contract_id for any other contract. This happens at the start of each season (for the previous season's contracts), as well as when an RFA/UFA/RD/RDI contract is dropped.
    #[sea_orm(string_value = "Expired")]
    Expired,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "auction::Entity")]
    Auction,
    #[sea_orm(has_one = "transaction::Entity")]
    DroppedContractTransaction,
    #[sea_orm(
        belongs_to = "league::Entity",
        from = "Column::LeagueId",
        to = "league::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    League,
    #[sea_orm(
        belongs_to = "league_player::Entity",
        from = "Column::LeaguePlayerId",
        to = "league_player::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    LeaguePlayer,
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::OriginalContractId",
        to = "Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    OriginalContract,
    #[sea_orm(
        belongs_to = "player::Entity",
        from = "Column::PlayerId",
        to = "player::Column::Id",
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
    #[sea_orm(has_one = "rookie_draft_selection::Entity")]
    RookieDraftSelection,
    #[sea_orm(
        belongs_to = "team::Entity",
        from = "Column::TeamId",
        to = "team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
    #[sea_orm(has_many = "trade_asset::Entity")]
    TradeAsset,
}

impl Related<auction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Auction.def()
    }
}

impl Related<league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<league_player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LeaguePlayer.def()
    }
}

impl Related<player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl Related<rookie_draft_selection::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RookieDraftSelection.def()
    }
}

impl Related<team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl Related<trade_asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAsset.def()
    }
}

impl Related<transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DroppedContractTransaction.def()
    }
}

#[derive(Debug)]
pub struct OriginalContract;
impl Linked for OriginalContract {
    type FromEntity = Entity;
    type ToEntity = Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::OriginalContract.def()]
    }
}

#[derive(Debug)]
pub struct PreviousContract;
impl Linked for PreviousContract {
    type FromEntity = Entity;
    type ToEntity = Entity;

    fn link(&self) -> Vec<RelationDef> {
        vec![Relation::PreviousContract.def()]
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, is_insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        non_original_contract_requires_previous_contract(&self)?;
        original_contract_requires_unset_previous_contract(&self)?;
        validate_contract_years_by_type(&self)?;
        validate_player_or_league_player_id(&self)?;

        if !is_insert {
            update_requires_original_contract(&self)?;
        } else {
            non_original_contract_requires_original_contract(&self)?;
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

fn non_original_contract_requires_original_contract(model: &ActiveModel) -> Result<(), DbErr> {
    if model.previous_contract_id.is_set() && model.original_contract_id.is_not_set() {
        Err(DbErr::Custom(format!("This contract (id={}, previous_contract_id={:?}) is missing a reference to the original contract for this player.", model.id.as_ref(), model.previous_contract_id.as_ref())))
    } else {
        Ok(())
    }
}

fn original_contract_requires_unset_previous_contract(model: &ActiveModel) -> Result<(), DbErr> {
    if model.previous_contract_id.is_set()
        && model.original_contract_id.is_set()
        && model.id.is_set()
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

fn validate_player_or_league_player_id(model: &ActiveModel) -> Result<(), DbErr> {
    let maybe_player_id = if model.player_id.is_not_set() {
        &None
    } else {
        model.player_id.as_ref()
    };
    let maybe_league_player_id = if model.league_player_id.is_not_set() {
        &None
    } else {
        model.league_player_id.as_ref()
    };

    if maybe_player_id.is_none() && maybe_league_player_id.is_none() {
        Err(DbErr::Custom(format!(
            "At least one of [player_id, league_player_id] must be set. Model:\n{:#?}",
            model
        )))
    } else {
        Ok(())
    }
}
