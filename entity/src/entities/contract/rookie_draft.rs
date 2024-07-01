use sea_orm::ActiveValue;

use super::{ActiveModel, ContractKind};

pub fn new_contract_from_rookie_draft(
    league_id: i64,
    end_of_season_year: i16,
    team_id: i64,
    salary: i16,
    player_id: i64,
    is_league_player: bool,
) -> ActiveModel {
    let mut model = ActiveModel {
        id: ActiveValue::NotSet,
        year_number: ActiveValue::Set(1),
        kind: ActiveValue::Set(ContractKind::RookieDevelopment),
        is_ir: ActiveValue::Set(false),
        salary: ActiveValue::Set(salary),
        end_of_season_year: ActiveValue::Set(end_of_season_year),
        status: ActiveValue::Set(super::ContractStatus::Active),
        league_id: ActiveValue::Set(league_id),
        league_player_id: ActiveValue::NotSet,
        player_id: ActiveValue::NotSet,
        previous_contract_id: ActiveValue::NotSet,
        original_contract_id: ActiveValue::NotSet,
        team_id: ActiveValue::Set(Some(team_id)),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    if is_league_player {
        model.league_player_id = ActiveValue::Set(Some(player_id));
    } else {
        model.player_id = ActiveValue::Set(Some(player_id));
    }

    model
}
