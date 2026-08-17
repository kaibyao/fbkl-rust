//! Cover for the keeper-deadline snapshot of each RFA's original owner (rules §14.4, §15.4.2).
//!
//! The kind change is annual advancement's job and is unit-tested there. What this checks is the
//! part only the keeper deadline can do: remember who owned the player at that moment, once per
//! RFA and never for a UFA.
//!
//! It lives in `jobs` because that is where the scratch database is set up.

use fbkl_entity::{
    contract::{self, ContractKind, ContractStatus},
    contract_queries,
    rfa_resolution::RfaResolutionStatus,
    rfa_resolution_queries,
    sea_orm::ActiveValue,
};
use fbkl_logic::deadline_processing::seed_rfa_resolutions;
use fbkl_test_support::TestLeague;

#[tokio::test]
async fn seeds_one_resolution_per_rfa_and_none_for_a_ufa() {
    let Some(league) = TestLeague::create("rfa_designation_seed", 2026).await else {
        return;
    };
    let rfa_player_id = league.add_veteran_player("Restricted Vet").await;
    let rfa_contract = league
        .add_owned_contract(
            rfa_player_id,
            ContractKind::RestrictedFreeAgent,
            7,
            league.team_id,
        )
        .await;
    let ufa_player_id = league.add_veteran_player("Unrestricted Vet").await;
    league
        .add_owned_contract(
            ufa_player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            9,
            league.team_id,
        )
        .await;
    let unowned_rfa_player_id = league.add_veteran_player("Unowned Restricted Vet").await;
    let unowned_rfa_contract = league
        .add_unowned_contract(unowned_rfa_player_id, ContractKind::RestrictedFreeAgent, 5)
        .await;

    let seeded = seed_rfa_resolutions(league.league_id, 2026, &league.db)
        .await
        .unwrap();

    assert_eq!(seeded.len(), 1);
    let seeded_resolution = &seeded[0];
    assert_eq!(seeded_resolution.rfa_contract_id, rfa_contract.id);
    assert_eq!(seeded_resolution.original_owner_team_id, league.team_id);
    assert_eq!(
        seeded_resolution.status,
        RfaResolutionStatus::AwaitingAuction
    );
    assert_eq!(seeded_resolution.auction_id, None);
    assert_eq!(seeded_resolution.raise_deadline_at, None);

    assert!(
        rfa_resolution_queries::find_rfa_resolution_for_contract(
            unowned_rfa_contract.id,
            &league.db
        )
        .await
        .unwrap()
        .is_none()
    );

    // Re-processing the keeper deadline must not fork a player's resolution.
    let seeded_again = seed_rfa_resolutions(league.league_id, 2026, &league.db)
        .await
        .unwrap();
    assert!(seeded_again.is_empty());
    let season_resolutions = rfa_resolution_queries::find_rfa_resolutions_for_league_season(
        league.league_id,
        2026,
        &league.db,
    )
    .await
    .unwrap();
    assert_eq!(season_resolutions.len(), 1);
}

/// The whole designation chain: a third-year rookie advances into a restricted free agent, and the
/// keeper deadline then records the team that still holds him.
#[tokio::test]
async fn an_advanced_third_year_rookie_is_seeded_with_its_owner() {
    let Some(league) = TestLeague::create("rfa_designation_advanced_rookie", 2026).await else {
        return;
    };
    let player_id = league.add_veteran_player("Third Year Rookie").await;
    let rookie_contract = contract_queries::create_new_contract(
        contract::ActiveModel {
            year_number: ActiveValue::Set(3),
            kind: ActiveValue::Set(ContractKind::Rookie),
            is_ir: ActiveValue::Set(false),
            salary: ActiveValue::Set(5),
            end_of_season_year: ActiveValue::Set(2025),
            status: ActiveValue::Set(ContractStatus::Active),
            league_id: ActiveValue::Set(league.league_id),
            player_id: ActiveValue::Set(Some(player_id)),
            team_id: ActiveValue::Set(Some(league.team_id)),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert the third-year rookie contract");

    let designated_contract = contract_queries::advance_contract(rookie_contract, &league.db)
        .await
        .expect("advance the rookie contract");
    assert_eq!(designated_contract.kind, ContractKind::RestrictedFreeAgent);
    assert_eq!(designated_contract.end_of_season_year, 2026);
    assert_eq!(designated_contract.team_id, Some(league.team_id));
    // 20% up from $5, rounded away from zero.
    assert_eq!(designated_contract.salary, 6);

    let seeded = seed_rfa_resolutions(league.league_id, 2026, &league.db)
        .await
        .expect("seed RFA resolutions");
    assert_eq!(seeded.len(), 1);
    assert_eq!(seeded[0].rfa_contract_id, designated_contract.id);
    assert_eq!(seeded[0].original_owner_team_id, league.team_id);
    assert_eq!(seeded[0].status, RfaResolutionStatus::AwaitingAuction);
}
