use async_graphql::{Context, Error, Object, Result};
use fbkl_entity::{
    contract::{self, ContractKind, ContractStatus},
    league_player_queries::find_league_player_by_id,
    player_queries::find_player_by_id,
    sea_orm::DatabaseConnection,
};

use crate::graphql::player::{LeagueOrRealPlayer, LeaguePlayer, RealPlayer};

#[derive(Clone, Default)]
pub struct Contract {
    pub id: i64,
    pub year_number: i16,
    pub kind: ContractKind,
    pub is_ir: bool,
    pub salary: i16,
    pub end_of_season_year: i16,
    pub status: ContractStatus,
    pub league_player_id: Option<i64>,
    pub player_id: Option<i64>,
    pub team_id: Option<i64>,
}

impl Contract {
    pub fn from_model(entity: contract::Model) -> Self {
        Self {
            id: entity.id,
            year_number: entity.year_number,
            kind: entity.kind,
            is_ir: entity.is_ir,
            salary: entity.salary,
            end_of_season_year: entity.end_of_season_year,
            status: entity.status,
            league_player_id: entity.league_player_id,
            player_id: entity.player_id,
            team_id: entity.team_id,
        }
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
        self.league_player_id
    }

    async fn player_id(&self) -> Option<i64> {
        self.player_id
    }

    async fn league_or_real_player(&self, ctx: &Context<'_>) -> Result<LeagueOrRealPlayer> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        if let Some(player_id) = self.player_id {
            let player = find_player_by_id(player_id, db).await?;
            Ok(LeagueOrRealPlayer::RealPlayer(RealPlayer::from_model(
                player,
            )))
        } else if let Some(league_player_id) = self.league_player_id {
            let league_player = find_league_player_by_id(league_player_id, db).await?;
            Ok(LeagueOrRealPlayer::LeaguePlayer(LeaguePlayer::from_model(
                league_player,
            )))
        } else {
            Err(Error::new("No player or league player found"))
        }
    }

    async fn team_id(&self) -> Option<i64> {
        self.team_id
    }
}
