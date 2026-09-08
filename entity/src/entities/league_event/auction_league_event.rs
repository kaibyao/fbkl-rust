use crate::{deadline, league_event};
use sea_orm::ActiveValue;

use super::LeagueEventKind;

pub fn new_auction_league_event(deadline_model: &deadline::Model) -> league_event::ActiveModel {
    league_event::ActiveModel {
        end_of_season_year: ActiveValue::Set(deadline_model.end_of_season_year),
        kind: ActiveValue::Set(LeagueEventKind::AuctionDone),
        league_id: ActiveValue::Set(deadline_model.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        ..Default::default()
    }
}
