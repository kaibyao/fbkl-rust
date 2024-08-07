use sea_orm::ActiveValue;

use crate::contract::{self, ContractKind, ContractStatus};

pub fn new_contract_for_auction(
    league_id: i64,
    end_of_season_year: i16,
    player_id: i64,
) -> contract::ActiveModel {
    contract::ActiveModel {
        id: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
        year_number: ActiveValue::Set(1),
        kind: ActiveValue::Set(ContractKind::Veteran),
        is_ir: ActiveValue::Set(false),
        salary: ActiveValue::Set(1),
        end_of_season_year: ActiveValue::Set(end_of_season_year),
        status: ActiveValue::Set(ContractStatus::Active),
        league_id: ActiveValue::Set(league_id),
        league_player_id: ActiveValue::NotSet,
        player_id: ActiveValue::Set(Some(player_id)),
        previous_contract_id: ActiveValue::NotSet,
        original_contract_id: ActiveValue::NotSet,
        team_id: ActiveValue::NotSet,
    }
}
