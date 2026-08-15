//! Cover for the compensation picks a declining original owner can be paid with (rules §15.2).
//!
//! The tier maths is unit-tested in `constants`. What needs a database is the subtraction around
//! it: the winner's own picks only, no round worse than the tier, and nothing the winner picked up
//! in a trade announced after the winning bid (§15.2.2).
//!
//! It lives in `jobs` because that is where the scratch database is set up.

use fbkl_entity::{
    contract::ContractKind,
    rfa_resolution::RfaResolutionStatus,
    rfa_resolution_queries::{self, NewRfaResolution},
    sea_orm::{ActiveModelTrait, ActiveValue},
};
use fbkl_logic::deadline_processing::compute_eligible_compensation_picks;
use fbkl_test_support::{TestLeague, central};

#[tokio::test]
async fn eligible_picks_drop_worse_rounds_other_owners_and_post_bid_acquisitions() {
    let Some(league) = TestLeague::create("rfa_compensation_picks", 2026).await else {
        return;
    };
    let original_owner_team_id = league.team_id;
    let winning_team_id = league.add_team("Winning team").await;
    let accepting_team_user_id = league
        .add_team_user(fbkl_entity::team_user::LeagueRole::TeamOwner)
        .await
        .id;
    let final_bid_at = central("2026-09-20T12:00:00");

    let held_first_round = league.add_draft_pick(1, winning_team_id).await;
    let held_fifth_round = league.add_draft_pick(5, winning_team_id).await;
    let original_owners_first_round = league.add_draft_pick(1, original_owner_team_id).await;
    let second_round_bought_after_bid = league.add_draft_pick(2, original_owner_team_id).await;
    let third_round_bought_before_bid = league.add_draft_pick(3, original_owner_team_id).await;

    league
        .add_completed_pick_trade(
            &third_round_bought_before_bid,
            winning_team_id,
            accepting_team_user_id,
            central("2026-09-19T09:00:00"),
        )
        .await;
    league
        .add_completed_pick_trade(
            &second_round_bought_after_bid,
            winning_team_id,
            accepting_team_user_id,
            central("2026-09-21T09:00:00"),
        )
        .await;

    let player_id = league.add_veteran_player("Restricted Vet").await;
    let rfa_contract = league
        .add_owned_contract(player_id, ContractKind::RestrictedFreeAgent, 7)
        .await;
    let mut resolution = rfa_resolution_queries::insert_rfa_resolution(
        NewRfaResolution {
            league_id: league.league_id,
            end_of_season_year: 2026,
            rfa_contract_id: rfa_contract.id,
            original_owner_team_id,
            auction_id: None,
            winning_team_id: Some(winning_team_id),
            final_bid: Some(20),
            final_bid_at: Some(final_bid_at),
            status: RfaResolutionStatus::AwaitingMatch,
            raise_deadline_at: Some(final_bid_at),
        },
        &league.db,
    )
    .await
    .unwrap();

    // $20 sits in the $19-$27 tier, so a 3rd-round pick or better settles it.
    let eligible = compute_eligible_compensation_picks(&resolution, &league.db)
        .await
        .unwrap();
    let eligible_ids: Vec<i64> = eligible.iter().map(|pick| pick.id).collect();
    assert_eq!(
        eligible_ids,
        vec![held_first_round.id, third_round_bought_before_bid.id],
        "expected the winner's own 1st and the 3rd it held before the bid, best round first"
    );
    for excluded in [
        held_fifth_round.id,
        original_owners_first_round.id,
        second_round_bought_after_bid.id,
    ] {
        assert!(!eligible_ids.contains(&excluded));
    }

    // A raise re-prices the tier: $42 needs a 1st-round pick, which drops the 3rd.
    resolution.raised_bid = Some(42);
    let eligible_after_raise = compute_eligible_compensation_picks(&resolution, &league.db)
        .await
        .unwrap();
    assert_eq!(
        eligible_after_raise
            .iter()
            .map(|pick| pick.id)
            .collect::<Vec<i64>>(),
        vec![held_first_round.id]
    );

    // With that 1st traded away the winner owes a pick he cannot pay (rules §15.3.3).
    let mut first_round_to_trade: fbkl_entity::draft_pick::ActiveModel = held_first_round.into();
    first_round_to_trade.current_owner_team_id = ActiveValue::Set(original_owner_team_id);
    first_round_to_trade.update(&league.db).await.unwrap();

    assert!(
        compute_eligible_compensation_picks(&resolution, &league.db)
            .await
            .unwrap()
            .is_empty()
    );
}
