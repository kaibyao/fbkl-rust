use async_graphql::{Context, Object, Result, Union, dataloader::DataLoader};
use color_eyre::eyre::eyre;
use fbkl_entity::{league_player, player};

use crate::{
    error::FbklError,
    graphql::{PlayerLoader, PositionLoader, RealTeamLoader},
};

#[derive(Debug, Clone, Eq, PartialEq, Union)]
pub enum LeagueOrRealPlayer {
    LeaguePlayer(LeaguePlayer),
    RealPlayer(RealPlayer),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LeaguePlayer {
    pub id: i64,
    pub is_rdi_eligible: bool,
    pub name: String,
    pub real_player_id: Option<i64>,
    // pub real_player: Option<RealPlayer>,
}

impl LeaguePlayer {
    pub fn from_model(entity: league_player::Model) -> Self {
        Self {
            id: entity.id,
            is_rdi_eligible: entity.is_rdi_eligible,
            name: entity.name,
            real_player_id: entity.real_player_id,
            // real_player: None,
        }
    }
}

#[Object]
impl LeaguePlayer {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn is_rdi_eligible(&self) -> bool {
        self.is_rdi_eligible
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn real_player_id(&self) -> Option<i64> {
        self.real_player_id
    }

    async fn real_player(&self, ctx: &Context<'_>) -> Result<Option<RealPlayer>, FbklError> {
        match self.real_player_id {
            Some(real_player_id) => {
                let real_player = ctx
                    .data_unchecked::<DataLoader<PlayerLoader>>()
                    .load_one(real_player_id)
                    .await
                    .map_err(|error| eyre!("{error}"))?
                    .ok_or_else(|| eyre!("player {real_player_id} not found"))?;
                Ok(Some(RealPlayer::from_model(real_player)))
            }
            None => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RealPlayer {
    pub id: i64,
    pub is_rdi_eligible: bool,
    pub name: String,
    pub photo_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub position_id: i32,
    // pub position: String,
    pub real_team_id: i64,
    // pub real_team_name: String,
}

impl RealPlayer {
    pub fn from_model(entity: player::Model) -> Self {
        Self {
            id: entity.id,
            is_rdi_eligible: entity.is_rdi_eligible,
            name: entity.name,
            photo_url: entity.photo_url,
            thumbnail_url: entity.thumbnail_url,
            position_id: entity.position_id,
            // position: "".to_string(),
            real_team_id: entity.current_real_team_id,
            // real_team_name: "".to_string(),
        }
    }
}

#[Object]
impl RealPlayer {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn is_rdi_eligible(&self) -> bool {
        self.is_rdi_eligible
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn photo_url(&self) -> Option<String> {
        self.photo_url.clone()
    }

    async fn thumbnail_url(&self) -> Option<String> {
        self.thumbnail_url.clone()
    }

    async fn position_id(&self) -> i32 {
        self.position_id
    }

    async fn position(&self, ctx: &Context<'_>) -> Result<String, FbklError> {
        let position = ctx
            .data_unchecked::<DataLoader<PositionLoader>>()
            .load_one(self.position_id)
            .await
            .map_err(|error| eyre!("{error}"))?
            .ok_or_else(|| eyre!("position {} not found", self.position_id))?;
        Ok(position.name)
    }

    async fn real_team_id(&self) -> i64 {
        self.real_team_id
    }

    async fn real_team_name(&self, ctx: &Context<'_>) -> Result<String, FbklError> {
        let real_team = ctx
            .data_unchecked::<DataLoader<RealTeamLoader>>()
            .load_one(self.real_team_id)
            .await
            .map_err(|error| eyre!("{error}"))?
            .ok_or_else(|| eyre!("real team {} not found", self.real_team_id))?;
        Ok(real_team.name)
    }
}
