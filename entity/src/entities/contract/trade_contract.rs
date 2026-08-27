use sea_orm::ActiveValue;

use crate::contract;

pub fn trade_contract_to_team(
    contract_model: &contract::Model,
    new_team_id: i64,
) -> contract::ActiveModel {
    let mut new_contract: contract::ActiveModel = contract_model.clone().into();
    new_contract.id = ActiveValue::NotSet;
    new_contract.team_id = ActiveValue::Set(Some(new_team_id));
    new_contract.previous_contract_id = ActiveValue::Set(Some(contract_model.id));
    // Rules §10.1.2: IR does not travel with a trade; the new team must re-accommodate the player.
    new_contract.is_ir = ActiveValue::Set(false);

    new_contract
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use chrono::{DateTime, FixedOffset};
    use sea_orm::ActiveValue;

    use crate::contract::{self, ContractKind, ContractStatus};

    use super::trade_contract_to_team;

    static NOW: LazyLock<DateTime<FixedOffset>> = LazyLock::new(|| {
        DateTime::parse_from_str("2023 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
            .unwrap()
    });

    #[test]
    fn traded_ir_contract_lands_without_ir() {
        let ir_contract = contract::Model {
            id: 1,
            kind: ContractKind::Veteran,
            year_number: 1,
            salary: 10,
            is_ir: true,
            end_of_season_year: 2023,
            league_id: 1,
            league_player_id: None,
            player_id: Some(1),
            original_contract_id: Some(1),
            previous_contract_id: None,
            team_id: Some(1),
            status: ContractStatus::Active,
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
        };

        let traded_contract = trade_contract_to_team(&ir_contract, 2);

        assert_eq!(traded_contract.is_ir, ActiveValue::Set(false));
        assert_eq!(traded_contract.team_id, ActiveValue::Set(Some(2)));
        assert_eq!(
            traded_contract.previous_contract_id,
            ActiveValue::Set(Some(1))
        );
    }
}
