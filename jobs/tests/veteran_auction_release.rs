//! End-to-end cover for the veteran auction's assemble -> release path (rules §6.3), the part no
//! pure-function test can reach: the pooled contract is written by one call and re-read by the next.

use fbkl_entity::{
    auction::{self, AuctionStatus},
    contract::ContractKind,
    deadline::DeadlineKind,
};
use fbkl_jobs::run_veteran_auction_release_tick;
use fbkl_logic::auction::assemble_veteran_auction_pool;
use fbkl_test_support::{TestLeague, central};

const END_OF_SEASON_YEAR: i16 = 2026;
const TIER_MIN_BID_AMOUNTS: [i16; 4] = [20, 15, 10, 5];

/// A never-owned veteran has no contract at all until the pool creates one, so his auction is the
/// only one that exercises the created-contract branch on both the assemble and the release side.
#[tokio::test]
async fn a_pooled_never_owned_veteran_opens_at_his_tier_minimum() {
    let Some(league) = TestLeague::create("veteran_auction_release", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonFinalRosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;

    let never_owned_player_id = league.add_veteran_player("Never Owned").await;
    league.add_ranked_players(&[never_owned_player_id]).await;
    let rfa_player_id = league.add_veteran_player("Restricted Vet").await;
    let ufa_player_id = league.add_veteran_player("Unrestricted Vet").await;
    league
        .add_unowned_contract(rfa_player_id, ContractKind::RestrictedFreeAgent, 7)
        .await;
    league
        .add_unowned_contract(
            ufa_player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            12,
        )
        .await;

    let schedule_rows =
        assemble_veteran_auction_pool(league.league_id, END_OF_SEASON_YEAR, &league.db)
            .await
            .expect("assemble the veteran auction pool");
    assert_eq!(schedule_rows.len(), 3);

    // Past RFA week, so every row in this three-player pool is due at once.
    let summary = run_veteran_auction_release_tick(&league.db, central("2025-09-10T12:00:00"))
        .await
        .expect("run the release tick");
    assert_eq!(summary.errors, 0);

    // The never-owned veteran is the whole ranked list, so he takes the top tier.
    let never_owned_auction = open_auction_for(&league, never_owned_player_id).await;
    assert_eq!(never_owned_auction.status, AuctionStatus::Open);
    assert_eq!(
        never_owned_auction.minimum_bid_amount,
        TIER_MIN_BID_AMOUNTS[0]
    );

    // RFA/UFA rows keep opening at their carry salary rather than a tier value (§15.3.1, §16).
    assert_eq!(
        open_auction_for(&league, rfa_player_id)
            .await
            .minimum_bid_amount,
        7
    );
    assert_eq!(
        open_auction_for(&league, ufa_player_id)
            .await
            .minimum_bid_amount,
        12
    );
}

/// Rules §6.3.4-.5 vs §15.3.1: the daily tier slide is the ladder's clock for the scheduled tier
/// pool only. An RFA opens at his 4th-year salary, so sliding him would price him under that floor.
#[tokio::test]
async fn the_tier_slide_leaves_an_unbid_rfa_at_his_carry_salary() {
    const RFA_CARRY_SALARY: i16 = 7;

    let Some(league) = TestLeague::create("veteran_auction_tier_slide", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonFinalRosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;

    let ranked_player_id = league.add_veteran_player("Ranked Vet").await;
    league.add_ranked_players(&[ranked_player_id]).await;
    let rfa_player_id = league.add_veteran_player("Restricted Vet").await;
    league
        .add_unowned_contract(
            rfa_player_id,
            ContractKind::RestrictedFreeAgent,
            RFA_CARRY_SALARY,
        )
        .await;

    assemble_veteran_auction_pool(league.league_id, END_OF_SEASON_YEAR, &league.db)
        .await
        .expect("assemble the veteran auction pool");

    // Past RFA week, so both rows open at once.
    let open_tick = central("2025-09-10T12:00:00");
    let summary = run_veteran_auction_release_tick(&league.db, open_tick)
        .await
        .expect("run the release tick");
    assert_eq!(summary.errors, 0);
    assert_eq!(
        open_auction_for(&league, ranked_player_id)
            .await
            .minimum_bid_amount,
        TIER_MIN_BID_AMOUNTS[0]
    );

    // A day later both auctions are still unbid, so the slide tick is eligible to move them.
    league.backdate_open_auctions(open_tick).await;
    let slide_tick = central("2025-09-11T12:00:00");
    let summary = run_veteran_auction_release_tick(&league.db, slide_tick)
        .await
        .expect("run the slide tick");
    assert_eq!(summary.errors, 0);

    assert_eq!(
        open_auction_for(&league, ranked_player_id)
            .await
            .minimum_bid_amount,
        TIER_MIN_BID_AMOUNTS[1],
        "a plain pooled auction slides one tier"
    );
    assert_eq!(
        open_auction_for(&league, rfa_player_id)
            .await
            .minimum_bid_amount,
        RFA_CARRY_SALARY,
        "the RFA minimum is his 4th-year salary, not the tier below it"
    );
}

async fn open_auction_for(league: &TestLeague, player_id: i64) -> auction::Model {
    league
        .find_veteran_auction(player_id)
        .await
        .unwrap_or_else(|| panic!("player {player_id} has no open auction"))
}
