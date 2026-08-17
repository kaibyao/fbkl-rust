//! Cover for the picks a bidder on a restricted free agent may name as compensation (rules §15.2).
//!
//! The tier maths is unit-tested in `constants`. What needs a database is the subtraction around
//! it: the bidder's own picks only, no round worse than the tier, and nothing he has already named
//! on another restricted free agent (§15.3.3).
//!
//! It lives in `jobs` because that is where the scratch database is set up.

use fbkl_entity::{
    contract::ContractKind,
    rfa_resolution::RfaResolutionStatus,
    rfa_resolution_queries::{self, NewRfaCompensationPick, NewRfaResolution},
    sea_orm::{ActiveModelTrait, ActiveValue},
};
use fbkl_logic::deadline_processing::eligible_compensation_picks;
use fbkl_test_support::{TestLeague, central};

#[tokio::test]
async fn eligible_picks_drop_worse_rounds_other_owners_and_picks_already_named() {
    let Some(league) = TestLeague::create("rfa_compensation_picks", 2026).await else {
        return;
    };
    let original_owner_team_id = league.team_id;
    let bidding_team_id = league.add_team("Bidding team").await;

    let held_first_round = league.add_draft_pick(1, bidding_team_id).await;
    let held_third_round = league.add_draft_pick(3, bidding_team_id).await;
    let held_fifth_round = league.add_draft_pick(5, bidding_team_id).await;
    let original_owners_first_round = league.add_draft_pick(1, original_owner_team_id).await;

    let resolution = seed_resolution(&league, "Restricted Vet").await;
    let other_resolution = seed_resolution(&league, "Other Restricted Vet").await;

    // $20 sits in the $19-$27 tier, so a 3rd-round pick or better settles it.
    let eligible_ids = eligible_ids_for(&league, bidding_team_id, 3, resolution.id).await;
    assert_eq!(
        eligible_ids,
        vec![held_first_round.id, held_third_round.id],
        "expected the bidder's own 1st and 3rd, best round first"
    );
    for excluded in [held_fifth_round.id, original_owners_first_round.id] {
        assert!(!eligible_ids.contains(&excluded));
    }

    // A pick named on another restricted free agent cannot settle this one too (§15.3.3).
    rfa_resolution_queries::upsert_rfa_compensation_pick(
        NewRfaCompensationPick {
            rfa_resolution_id: other_resolution.id,
            required_round: 3,
            forfeited_draft_pick_id: held_third_round.id,
            to_team_id: original_owner_team_id,
            from_team_id: bidding_team_id,
        },
        &league.db,
    )
    .await
    .unwrap();
    assert_eq!(
        eligible_ids_for(&league, bidding_team_id, 3, resolution.id).await,
        vec![held_first_round.id]
    );

    // A raise re-prices the tier: $42 needs a 1st-round pick anyway.
    assert_eq!(
        eligible_ids_for(&league, bidding_team_id, 1, resolution.id).await,
        vec![held_first_round.id]
    );

    // With that 1st traded away the bidder owes a pick he cannot pay (rules §15.3.3).
    let mut first_round_to_trade: fbkl_entity::draft_pick::ActiveModel = held_first_round.into();
    first_round_to_trade.current_owner_team_id = ActiveValue::Set(original_owner_team_id);
    first_round_to_trade.update(&league.db).await.unwrap();

    assert!(
        eligible_ids_for(&league, bidding_team_id, 1, resolution.id)
            .await
            .is_empty()
    );
}

async fn eligible_ids_for(
    league: &TestLeague,
    team_id: i64,
    required_round: i16,
    excluded_rfa_resolution_id: i64,
) -> Vec<i64> {
    eligible_compensation_picks(
        league.league_id,
        2026,
        team_id,
        required_round,
        excluded_rfa_resolution_id,
        &league.db,
    )
    .await
    .unwrap()
    .iter()
    .map(|draft_pick| draft_pick.id)
    .collect()
}

/// A designated RFA still waiting on its auction, which is when a bid names its pick.
async fn seed_resolution(
    league: &TestLeague,
    player_name: &str,
) -> fbkl_entity::rfa_resolution::Model {
    let player_id = league.add_veteran_player(player_name).await;
    let rfa_contract = league
        .add_owned_contract(
            player_id,
            ContractKind::RestrictedFreeAgent,
            7,
            league.team_id,
        )
        .await;

    rfa_resolution_queries::insert_rfa_resolution(
        NewRfaResolution {
            league_id: league.league_id,
            end_of_season_year: 2026,
            rfa_contract_id: rfa_contract.id,
            original_owner_team_id: league.team_id,
            auction_id: None,
            winning_team_id: None,
            final_bid: None,
            final_bid_at: None,
            status: RfaResolutionStatus::AwaitingAuction,
            raise_deadline_at: Some(central("2026-09-20T12:00:00")),
        },
        &league.db,
    )
    .await
    .unwrap()
}
