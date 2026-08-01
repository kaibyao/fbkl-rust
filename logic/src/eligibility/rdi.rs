//! RDI eligibility validator (rules §11.3.1).
//!
//! Gates RD → RDI moves. The `is_rdi_eligible` bool on `player`/`league_player` is a cache this
//! validator can correct, so it is deliberately not an input here — callers pass the derived facts.
//!
//! RDI keys off the broader §3.1.3 "was on an NBA roster" fact, not the §3.1.2 pool pivot: §11.3.1
//! forces RDI→RD/1 the moment a player is "on an NBA roster / signed to an NBA contract", so a
//! rostered player who never appeared in a game is still rookie-draft-eligible yet already
//! RDI-ineligible. Both are asked as of the contract's own season, so replaying a 2021 RD→RDI move
//! is judged on what was true entering 2021, not on the player's career to date. §11.3.5's
//! mid-season grace period is why "before the season" is strict — see `was_on_nba_roster_before`.

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
/// Rules §11.3.1, judged entering the contract's season: the player must be rookie-draft-eligible,
/// must not have been on an NBA roster in an earlier season (broader than the pool pivot — see the
/// module docs), and must not have already been an RD contract at/after an in-season legalization.
#[instrument(skip(db))]
pub async fn validate_rdi_eligible<C>(
    contract_model: &contract::Model,
    player_facts: PlayerEligibilityFacts,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let season = contract_model.end_of_season_year;
    ensure!(
        classify_player(player_facts, season) == EligibilityClassification::RookieDraftEligible,
        "Contract (id = {}) player was not rookie-draft-eligible in {season}, so cannot move to RDI.",
        contract_model.id
    );
    ensure!(
        !player_facts.was_on_nba_roster_before(season),
        "Contract (id = {}) player was on an NBA roster before {season}, so cannot move to RDI.",
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
