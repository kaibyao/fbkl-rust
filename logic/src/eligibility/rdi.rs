//! RDI eligibility validator (rules §11.3.1).
//!
//! Gates RD → RDI moves. The `is_rdi_eligible` bool on `player`/`league_player` is a cache this
//! validator can correct, so it is deliberately not an input here — callers pass the derived facts.

use std::fmt::Debug;

use color_eyre::eyre::{Result, ensure};
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    player::EligibilityClassification,
    sea_orm::ConnectionTrait,
};
use tracing::instrument;

use super::{PlayerEligibilityFacts, classify_player};

/// Validates that a contract's player may move to Rookie Development International.
///
/// Rules §11.3.1: the player must be rookie-draft-eligible, must never have been on an NBA roster,
/// and must not have already been an RD contract at/after an in-season roster legalization.
#[instrument]
pub async fn validate_rdi_eligible<C>(
    contract_model: &contract::Model,
    player_facts: PlayerEligibilityFacts,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    ensure!(
        classify_player(player_facts) == EligibilityClassification::RookieDraftEligible,
        "Contract (id = {}) player is not rookie-draft-eligible, so cannot move to RDI.",
        contract_model.id
    );
    ensure!(
        !player_facts.has_been_on_nba_roster,
        "Contract (id = {}) player has been on an NBA roster, so cannot move to RDI.",
        contract_model.id
    );

    let chain = contract_queries::find_contract_chain(contract_model.id, db).await?;
    ensure!(
        !has_legalized_rd_ancestor(&chain, contract_model.end_of_season_year),
        "Contract (id = {}) was already a post-legalization RD contract, so cannot move to RDI.",
        contract_model.id
    );

    Ok(())
}

/// Proxy for "was RD at/after an in-season roster legalization": an RD contract past year 1, or an
/// RD contract from an earlier season (which necessarily crossed that season's legalization).
fn has_legalized_rd_ancestor(chain: &[contract::Model], end_of_season_year: i16) -> bool {
    chain.iter().any(|chain_contract| {
        chain_contract.kind == ContractKind::RookieDevelopment
            && (chain_contract.year_number > 1
                || chain_contract.end_of_season_year < end_of_season_year)
    })
}

#[cfg(test)]
mod tests {
    use fbkl_entity::contract::{ContractKind, ContractStatus};

    use super::has_legalized_rd_ancestor;

    fn contract(
        id: i64,
        kind: ContractKind,
        year_number: i16,
        end_of_season_year: i16,
    ) -> fbkl_entity::contract::Model {
        fbkl_entity::contract::Model {
            id,
            year_number,
            kind,
            is_ir: false,
            salary: 10,
            end_of_season_year,
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
    fn fresh_rd_year_one_chain_is_allowed() {
        let chain = [contract(1, ContractKind::RookieDevelopment, 1, 2025)];

        assert!(!has_legalized_rd_ancestor(&chain, 2025));
    }

    #[test]
    fn rd_ancestor_from_earlier_season_is_rejected() {
        let chain = [
            contract(1, ContractKind::RookieDevelopment, 1, 2024),
            contract(2, ContractKind::RookieDevelopment, 1, 2025),
        ];

        assert!(has_legalized_rd_ancestor(&chain, 2025));
    }

    #[test]
    fn rd_past_year_one_is_rejected() {
        let chain = [contract(1, ContractKind::RookieDevelopment, 2, 2025)];

        assert!(has_legalized_rd_ancestor(&chain, 2025));
    }

    #[test]
    fn non_rd_ancestors_are_ignored() {
        let chain = [
            contract(1, ContractKind::Rookie, 2, 2024),
            contract(2, ContractKind::RookieDevelopment, 1, 2025),
        ];

        assert!(!has_legalized_rd_ancestor(&chain, 2025));
    }
}
