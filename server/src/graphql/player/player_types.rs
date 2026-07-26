use async_graphql::{Context, Object, Result, SimpleObject, Union, dataloader::DataLoader};
use color_eyre::eyre::eyre;
use fbkl_entity::{
    contract::RelatedPlayer,
    league_player,
    player::{self, EligibilityClassification, NbaRosterSource},
};
use fbkl_logic::eligibility::{PlayerEligibilityFacts, classify_player};

use crate::{
    error::FbklError,
    graphql::{PlayerLoader, PositionLoader, RealTeamLoader},
};

#[derive(Debug, Clone, Eq, PartialEq, Union)]
pub enum LeagueOrRealPlayer {
    LeaguePlayer(LeaguePlayer),
    RealPlayer(RealPlayer),
}

impl LeagueOrRealPlayer {
    /// Pool builders and contracts both hand back entity's `RelatedPlayer`. `end_of_season_year` is
    /// the season the classification is asked about — the pool's season, or the contract's.
    pub fn from_related_player(related_player: RelatedPlayer, end_of_season_year: i16) -> Self {
        match related_player {
            RelatedPlayer::LeaguePlayer(model) => {
                Self::LeaguePlayer(LeaguePlayer::from_model(model, end_of_season_year))
            }
            RelatedPlayer::Player(model) => {
                Self::RealPlayer(RealPlayer::from_model(model, end_of_season_year))
            }
        }
    }
}

/// The spec-10 eligibility columns shared by both player types, plus the classification derived for
/// `classifiedForEndOfSeasonYear` — eligibility is per-season, so the answer is stamped with the
/// season it answers for.
#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct PlayerEligibility {
    pub classification: EligibilityClassification,
    pub classified_for_end_of_season_year: i16,
    /// §3.1.2, over the player's whole career. Narrowed to a season by `nbaFirstSeason...`.
    pub has_played_nba_game: bool,
    /// The season the player first appeared in NBA data. `<=` a season is the §3.1.3 RDI fact.
    pub nba_first_season_end_of_season_year: Option<i16>,
    pub nba_roster_source: NbaRosterSource,
    pub nba_roster_asof: Option<String>,
    pub eligibility_override: Option<EligibilityClassification>,
}

impl PlayerEligibility {
    fn from_league_player(entity: &league_player::Model, end_of_season_year: i16) -> Self {
        Self {
            classification: classify_player(
                PlayerEligibilityFacts::from(entity),
                end_of_season_year,
            ),
            classified_for_end_of_season_year: end_of_season_year,
            has_played_nba_game: entity.has_played_nba_game,
            nba_first_season_end_of_season_year: entity.nba_first_season_end_of_season_year,
            nba_roster_source: entity.nba_roster_source,
            nba_roster_asof: entity.nba_roster_asof.map(|asof| asof.to_rfc3339()),
            eligibility_override: entity.eligibility_override,
        }
    }

    fn from_player(entity: &player::Model, end_of_season_year: i16) -> Self {
        Self {
            classification: classify_player(
                PlayerEligibilityFacts::from(entity),
                end_of_season_year,
            ),
            classified_for_end_of_season_year: end_of_season_year,
            has_played_nba_game: entity.has_played_nba_game,
            nba_first_season_end_of_season_year: entity.nba_first_season_end_of_season_year,
            nba_roster_source: entity.nba_roster_source,
            nba_roster_asof: entity.nba_roster_asof.map(|asof| asof.to_rfc3339()),
            eligibility_override: entity.eligibility_override,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaguePlayer {
    pub id: i64,
    pub is_rdi_eligible: bool,
    pub name: String,
    pub real_player_id: Option<i64>,
    pub eligibility: PlayerEligibility,
}

impl LeaguePlayer {
    pub fn from_model(entity: league_player::Model, end_of_season_year: i16) -> Self {
        let eligibility = PlayerEligibility::from_league_player(&entity, end_of_season_year);
        Self {
            id: entity.id,
            is_rdi_eligible: entity.is_rdi_eligible,
            name: entity.name,
            real_player_id: entity.real_player_id,
            eligibility,
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

    async fn eligibility(&self) -> PlayerEligibility {
        self.eligibility.clone()
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
                Ok(Some(RealPlayer::from_model(
                    real_player,
                    self.eligibility.classified_for_end_of_season_year,
                )))
            }
            None => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealPlayer {
    pub id: i64,
    pub is_rdi_eligible: bool,
    pub name: String,
    pub photo_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub position_id: i32,
    // pub position: String,
    pub real_team_id: i64,
    pub eligibility: PlayerEligibility,
    // pub real_team_name: String,
}

impl RealPlayer {
    pub fn from_model(entity: player::Model, end_of_season_year: i16) -> Self {
        let eligibility = PlayerEligibility::from_player(&entity, end_of_season_year);
        Self {
            id: entity.id,
            is_rdi_eligible: entity.is_rdi_eligible,
            name: entity.name,
            photo_url: entity.photo_url,
            thumbnail_url: entity.thumbnail_url,
            position_id: entity.position_id,
            // position: "".to_string(),
            real_team_id: entity.current_real_team_id,
            eligibility,
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

    async fn eligibility(&self) -> PlayerEligibility {
        self.eligibility.clone()
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
