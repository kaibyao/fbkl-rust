mod move_rd_contract_to_international;
mod move_rdi_contract_from_international;
mod rdi_team_update;

pub use move_rd_contract_to_international::*;
pub use move_rdi_contract_from_international::*;

use color_eyre::eyre::{Result, ensure};
use fbkl_entity::contract::{self, ContractKind};

/// Guards an RD↔RDI move against being applied to the wrong contract kind (rules §11.3.1).
fn validate_contract_kind(contract_model: &contract::Model, expected: ContractKind) -> Result<()> {
    ensure!(
        contract_model.kind == expected,
        "Contract (id = {}) is a {:?} contract, but this move requires a {:?} contract.",
        contract_model.id,
        contract_model.kind,
        expected
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use fbkl_entity::contract::{ContractKind, ContractStatus, Model};

    use super::validate_contract_kind;

    fn contract(kind: ContractKind) -> Model {
        Model {
            id: 1,
            year_number: 1,
            kind,
            is_ir: false,
            salary: 10,
            end_of_season_year: 2025,
            status: ContractStatus::Active,
            league_id: 1,
            league_player_id: None,
            player_id: Some(1),
            previous_contract_id: None,
            original_contract_id: Some(1),
            team_id: Some(7),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn already_rdi_cannot_move_to_rdi() {
        let contract_model = contract(ContractKind::RookieDevelopmentInternational);

        assert!(
            validate_contract_kind(&contract_model, ContractKind::RookieDevelopment).is_err(),
            "an RDI contract must not pass the RD → RDI guard"
        );
    }

    #[test]
    fn veteran_cannot_move_out_of_rdi() {
        let contract_model = contract(ContractKind::Veteran);

        assert!(
            validate_contract_kind(
                &contract_model,
                ContractKind::RookieDevelopmentInternational
            )
            .is_err(),
            "a veteran contract must not pass the RDI → RD guard"
        );
    }

    #[test]
    fn matching_kind_passes() {
        let contract_model = contract(ContractKind::RookieDevelopment);

        assert!(validate_contract_kind(&contract_model, ContractKind::RookieDevelopment).is_ok());
    }
}
