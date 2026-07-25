use color_eyre::Result;
use fbkl_constants::FREE_AGENCY_TEAM;
use sea_orm::ConnectionTrait;
use std::fmt::Debug;
use tracing::instrument;

use crate::contract::{self, RelatedPlayer};

pub struct ContractUpdatePlayerData {
    pub player_name: String,
    pub real_team_abbr: String,
    pub real_team_name: String,
}

impl ContractUpdatePlayerData {
    #[instrument]
    pub async fn from_contract_model<C>(contract_model: &contract::Model, db: &C) -> Result<Self>
    where
        C: ConnectionTrait + Debug,
    {
        let player_model = contract_model.get_player(db).await?;

        let data = match player_model {
            RelatedPlayer::LeaguePlayer(league_player_model) => Self {
                player_name: league_player_model.name,
                real_team_abbr: FREE_AGENCY_TEAM.abbr.to_string(),
                real_team_name: format!("{} {}", FREE_AGENCY_TEAM.city, FREE_AGENCY_TEAM.name),
            },
            RelatedPlayer::Player(player_model) => {
                let real_team_model = player_model.get_real_team(db).await?;
                Self {
                    player_name: player_model.name,
                    real_team_abbr: real_team_model.code,
                    real_team_name: format!("{} {}", real_team_model.city, real_team_model.name),
                }
            }
        };

        Ok(data)
    }
}
