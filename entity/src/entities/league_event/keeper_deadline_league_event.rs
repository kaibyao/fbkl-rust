use crate::{deadline, league_event};
use sea_orm::ActiveValue;

use super::LeagueEventKind;

pub fn new_keeper_deadline_league_event(
    keeper_deadline_model: &deadline::Model,
) -> league_event::ActiveModel {
    league_event::ActiveModel {
        end_of_season_year: ActiveValue::Set(keeper_deadline_model.end_of_season_year),
        kind: ActiveValue::Set(LeagueEventKind::PreseasonKeeper),
        league_id: ActiveValue::Set(keeper_deadline_model.league_id),
        deadline_id: ActiveValue::Set(keeper_deadline_model.id),
        ..Default::default()
    }
}
