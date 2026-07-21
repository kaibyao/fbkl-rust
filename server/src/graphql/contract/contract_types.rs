use async_graphql::{Context, Object, Result};
use color_eyre::eyre::eyre;
use fbkl_entity::{
    contract::{self, ContractKind, ContractStatus},
    league_player_queries::find_league_player_by_id,
    player_queries::find_player_by_id,
    sea_orm::DatabaseConnection,
};

use crate::{
    error::FbklError,
    graphql::player::{LeagueOrRealPlayer, LeaguePlayer, RealPlayer},
};

/// Exactly one player reference per contract; stored as an enum so the two-`Option` illegal
/// state (both set / neither set) can't be constructed. The scalar GraphQL fields derive from it.
#[derive(Clone, Copy)]
enum PlayerRef {
    League(i64),
    Real(i64),
}

#[derive(Clone)]
pub struct Contract {
    pub id: i64,
    pub year_number: i16,
    pub kind: ContractKind,
    pub is_ir: bool,
    pub salary: i16,
    pub end_of_season_year: i16,
    pub status: ContractStatus,
    player: PlayerRef,
    pub team_id: Option<i64>,
}

impl Contract {
    pub fn from_model(entity: &contract::Model) -> Result<Self, FbklError> {
        // Real player wins when both are set, matching the prior resolver's precedence.
        let player = match (entity.player_id, entity.league_player_id) {
            (Some(id), _) => PlayerRef::Real(id),
            (None, Some(id)) => PlayerRef::League(id),
            (None, None) => {
                return Err(eyre!("contract {} has no player or league player", entity.id).into());
            }
        };
        Ok(Self {
            id: entity.id,
            year_number: entity.year_number,
            kind: entity.kind,
            is_ir: entity.is_ir,
            salary: entity.salary,
            end_of_season_year: entity.end_of_season_year,
            status: entity.status,
            player,
            team_id: entity.team_id,
        })
    }
}

#[Object]
impl Contract {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn year_number(&self) -> i16 {
        self.year_number
    }

    async fn kind(&self) -> ContractKind {
        self.kind
    }

    async fn is_ir(&self) -> bool {
        self.is_ir
    }

    async fn salary(&self) -> i16 {
        self.salary
    }

    async fn end_of_season_year(&self) -> i16 {
        self.end_of_season_year
    }

    async fn status(&self) -> ContractStatus {
        self.status
    }

    async fn league_player_id(&self) -> Option<i64> {
        match self.player {
            PlayerRef::League(id) => Some(id),
            PlayerRef::Real(_) => None,
        }
    }

    async fn player_id(&self) -> Option<i64> {
        match self.player {
            PlayerRef::Real(id) => Some(id),
            PlayerRef::League(_) => None,
        }
    }

    async fn league_or_real_player(
        &self,
        ctx: &Context<'_>,
    ) -> Result<LeagueOrRealPlayer, FbklError> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        match self.player {
            PlayerRef::Real(player_id) => {
                let player = find_player_by_id(player_id, db).await?;
                Ok(LeagueOrRealPlayer::RealPlayer(RealPlayer::from_model(
                    player,
                )))
            }
            PlayerRef::League(league_player_id) => {
                let league_player = find_league_player_by_id(league_player_id, db).await?;
                Ok(LeagueOrRealPlayer::LeaguePlayer(LeaguePlayer::from_model(
                    league_player,
                )))
            }
        }
    }

    async fn team_id(&self) -> Option<i64> {
        self.team_id
    }
}
