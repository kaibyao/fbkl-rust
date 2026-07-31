//! The two per-season commissioner inputs the veteran auction needs (rules §6.3.6): the ordered
//! minimum-bid tiers and the ranked nomination list.
//!
//! Both are stored per league season and read by pool assembly, so entry has to be re-runnable —
//! a commissioner fixing a typo must not end up with two overlapping lists.

use fbkl_entity::{
    auction_schedule_queries::{find_min_bid_tiers, find_veteran_auction_ranked_player_ids},
    deadline::DeadlineKind,
};
use fbkl_logic::auction::assemble_veteran_auction_pool;
use fbkl_test_support::{TestLeague, central};

const END_OF_SEASON_YEAR: i16 = 2026;
const TIER_MIN_BID_AMOUNTS: [i16; 4] = [20, 15, 10, 5];

/// Assembly reads both inputs out of the database, so the tiers and the rank order a season was
/// given are exactly what its schedule rows carry.
#[tokio::test]
async fn pool_assembly_uses_the_configured_tiers_and_ranking() {
    let Some(league) =
        TestLeague::create("veteran_auction_season_config", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;

    let ranked_player_ids = [
        league.add_veteran_player("Best Vet").await,
        league.add_veteran_player("Second Vet").await,
        league.add_veteran_player("Third Vet").await,
        league.add_veteran_player("Fourth Vet").await,
    ];
    let unranked_player_id = league.add_veteran_player("Open Nomination Vet").await;
    league.add_ranked_players(&ranked_player_ids).await;

    let schedule_rows =
        assemble_veteran_auction_pool(league.league_id, END_OF_SEASON_YEAR, &league.db)
            .await
            .expect("assemble the veteran auction pool");
    assert_eq!(schedule_rows.len(), 5);

    // Four ranked players across four tiers: one per tier, best rank in the top tier.
    for (expected_rank, player_id) in ranked_player_ids.iter().enumerate() {
        let row = schedule_rows
            .iter()
            .find(|row| row.player_id == *player_id)
            .expect("ranked player is scheduled");
        assert_eq!(
            row.nomination_rank,
            Some(i16::try_from(expected_rank + 1).unwrap())
        );
        assert_eq!(row.min_bid_tier, i16::try_from(expected_rank).unwrap());
    }

    // An unranked pooled player is open-nomination at the bottom tier (§6.3.2).
    let unranked_row = schedule_rows
        .iter()
        .find(|row| row.player_id == unranked_player_id)
        .expect("unranked player is scheduled");
    assert_eq!(unranked_row.nomination_rank, None);
    assert_eq!(
        unranked_row.min_bid_tier,
        i16::try_from(TIER_MIN_BID_AMOUNTS.len() - 1).unwrap()
    );
}

/// Re-entering a season's config replaces it, so the commissioner can correct a list without
/// leaving the old one behind.
#[tokio::test]
async fn re_entering_season_config_replaces_it() {
    let Some(league) =
        TestLeague::create("veteran_auction_season_config_reentry", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    let first_player_id = league.add_veteran_player("First Vet").await;
    let second_player_id = league.add_veteran_player("Second Vet").await;

    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;
    league
        .add_ranked_players(&[first_player_id, second_player_id])
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;
    league
        .add_ranked_players(&[first_player_id, second_player_id])
        .await;

    let tiers = find_min_bid_tiers(league.league_id, END_OF_SEASON_YEAR, &league.db)
        .await
        .expect("find min bid tiers");
    assert_eq!(
        tiers
            .iter()
            .map(|tier| tier.min_bid_amount)
            .collect::<Vec<_>>(),
        TIER_MIN_BID_AMOUNTS
    );
    assert_eq!(
        find_veteran_auction_ranked_player_ids(league.league_id, END_OF_SEASON_YEAR, &league.db)
            .await
            .expect("find ranked players"),
        vec![first_player_id, second_player_id]
    );

    // A corrected entry wins outright rather than merging with what it replaces.
    league.add_min_bid_tiers(&[30, 10]).await;
    league.add_ranked_players(&[second_player_id]).await;
    assert_eq!(
        find_min_bid_tiers(league.league_id, END_OF_SEASON_YEAR, &league.db)
            .await
            .expect("find min bid tiers")
            .iter()
            .map(|tier| tier.min_bid_amount)
            .collect::<Vec<_>>(),
        vec![30, 10]
    );
    assert_eq!(
        find_veteran_auction_ranked_player_ids(league.league_id, END_OF_SEASON_YEAR, &league.db)
            .await
            .expect("find ranked players"),
        vec![second_player_id]
    );
}

/// Without a ranked list every pooled player would silently open at the bottom tier, so assembly
/// refuses to run instead.
#[tokio::test]
async fn pool_assembly_refuses_a_season_with_no_ranked_list() {
    let Some(league) =
        TestLeague::create("veteran_auction_season_config_unranked", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;
    league.add_veteran_player("Unranked Vet").await;

    let assembly_result =
        assemble_veteran_auction_pool(league.league_id, END_OF_SEASON_YEAR, &league.db).await;
    assert!(assembly_result.is_err(), "assembly needs a ranked list");
}
