//! Commissioner writes to the eligibility columns on `player` / `league_player` (spec 10).
//!
//! Two deliberately separate writes: the NBA *facts* (which the feed also sets) and the
//! commissioner's override of the *derived classification*. The pair is identical for both player
//! tables, so one macro generates both.

use chrono::Utc;
use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait};

use crate::player::{EligibilityClassification, NbaRosterSource};

macro_rules! eligibility_writes {
    ($entity:ident, $set_fact:ident, $set_override:ident) => {
        /// Corrects both NBA facts, marking the commissioner as their source.
        pub async fn $set_fact<C>(
            model: crate::$entity::Model,
            has_played_nba_game: bool,
            nba_first_season_end_of_season_year: Option<i16>,
            db: &C,
        ) -> Result<crate::$entity::Model>
        where
            C: ConnectionTrait,
        {
            let mut active_model: crate::$entity::ActiveModel = model.into();
            active_model.has_played_nba_game = ActiveValue::Set(has_played_nba_game);
            active_model.nba_first_season_end_of_season_year =
                ActiveValue::Set(nba_first_season_end_of_season_year);
            active_model.nba_roster_source =
                ActiveValue::Set(NbaRosterSource::CommissionerOverride);
            active_model.nba_roster_asof = ActiveValue::Set(Some(Utc::now().into()));
            Ok(active_model.update(db).await?)
        }

        /// Overrides (or with `None`, clears) the derived classification plus its audit trail.
        pub async fn $set_override<C>(
            model: crate::$entity::Model,
            classification: Option<EligibilityClassification>,
            reason: String,
            team_user_id: i64,
            db: &C,
        ) -> Result<crate::$entity::Model>
        where
            C: ConnectionTrait,
        {
            let mut active_model: crate::$entity::ActiveModel = model.into();
            active_model.eligibility_override = ActiveValue::Set(classification);
            active_model.eligibility_override_reason = ActiveValue::Set(Some(reason));
            active_model.eligibility_override_by_team_user_id =
                ActiveValue::Set(Some(team_user_id));
            active_model.eligibility_override_at = ActiveValue::Set(Some(Utc::now().into()));
            Ok(active_model.update(db).await?)
        }
    };
}

eligibility_writes!(
    player,
    set_player_nba_status,
    set_player_eligibility_override
);
eligibility_writes!(
    league_player,
    set_league_player_nba_status,
    set_league_player_eligibility_override
);
