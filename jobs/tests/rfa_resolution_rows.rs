//! Row-level cover for the RFA resolution tables (rules §15.2, §15.3): the migration, the entities
//! and the queries have to agree on every column, and nothing else exercises them until the
//! handshake logic lands.
//!
//! It lives in `jobs` because that is where the scratch-database harness is wired up; the code under
//! test is `fbkl_entity`.

use fbkl_entity::{
    contract::ContractKind,
    rfa_resolution::RfaResolutionStatus,
    rfa_resolution_queries::{self, NewRfaCompensationPick, NewRfaResolution},
};
use fbkl_test_support::{TestLeague, central};

#[tokio::test]
async fn rfa_resolution_rows_round_trip() {
    let Some(league) = TestLeague::create("rfa_resolution_rows", 2026).await else {
        return;
    };
    let player_id = league.add_veteran_player("Restricted Vet").await;
    let contract = league
        .add_unowned_contract(player_id, ContractKind::RestrictedFreeAgent, 7)
        .await;

    let inserted = rfa_resolution_queries::insert_rfa_resolution(
        NewRfaResolution {
            league_id: league.league_id,
            end_of_season_year: 2026,
            rfa_contract_id: contract.id,
            original_owner_team_id: league.team_id,
            auction_id: None,
            winning_team_id: Some(league.team_id),
            final_bid: Some(19),
            final_bid_at: Some(central("2025-09-10T12:00:00")),
            status: RfaResolutionStatus::AwaitingRaise,
            raise_deadline_at: Some(central("2025-09-12T12:00:00")),
        },
        &league.db,
    )
    .await
    .unwrap();

    let found = rfa_resolution_queries::find_rfa_resolution_for_contract(contract.id, &league.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, inserted.id);
    assert_eq!(found.status, RfaResolutionStatus::AwaitingRaise);
    assert_eq!(found.final_bid, Some(19));

    let expired = rfa_resolution_queries::find_rfa_resolutions_with_expired_window(
        central("2025-09-13T12:00:00"),
        &league.db,
    )
    .await
    .unwrap();
    assert_eq!(expired.len(), 1);

    let not_expired = rfa_resolution_queries::find_rfa_resolutions_with_expired_window(
        central("2025-09-11T12:00:00"),
        &league.db,
    )
    .await
    .unwrap();
    assert!(not_expired.is_empty());

    let updated = rfa_resolution_queries::finish_rfa_resolution(
        inserted.id,
        RfaResolutionStatus::Declined,
        central("2025-09-12T12:00:00"),
        &league.db,
    )
    .await
    .unwrap();
    assert_eq!(updated.status, RfaResolutionStatus::Declined);
    assert!(updated.resolved_at.is_some());

    let compensation = rfa_resolution_queries::insert_rfa_compensation_pick(
        NewRfaCompensationPick {
            rfa_resolution_id: inserted.id,
            required_round: 3,
            forfeited_draft_pick_id: None,
            to_team_id: league.team_id,
            from_team_id: league.team_id,
        },
        &league.db,
    )
    .await
    .unwrap();
    let found_compensation =
        rfa_resolution_queries::find_rfa_compensation_pick_for_resolution(inserted.id, &league.db)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(found_compensation.id, compensation.id);
    assert_eq!(found_compensation.required_round, 3);

    let season_rows = rfa_resolution_queries::find_rfa_resolutions_for_league_season(
        league.league_id,
        2026,
        &league.db,
    )
    .await
    .unwrap();
    assert_eq!(season_rows.len(), 1);
}
