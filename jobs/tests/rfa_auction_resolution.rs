//! Cover for the RFA auction's close -> resolution path (rules §6.5), which needs a database
//! because the close writes the pooled contract that the resolution re-reads.
//!
//! An RFA auction is the one auction whose close does not sign anybody: the original team may still
//! match. `resolve_rfa_auction_to_winning_bid` is the no-match outcome, called from the decline
//! branch of the handshake and covered here on its own.

use fbkl_entity::{
    auction::AuctionStatus,
    auction_queries,
    contract::{ContractKind, ContractStatus},
    deadline::DeadlineKind,
    team_user::LeagueRole,
};
use fbkl_logic::auction::{
    end_veteran_auction, resolve_rfa_auction_to_winning_bid, start_new_auction_for_nba_player,
};
use fbkl_test_support::{TestLeague, central};

const END_OF_SEASON_YEAR: i16 = 2026;
const RFA_CARRY_SALARY: i16 = 7;
const WINNING_BID: i16 = 19;

/// Sets up a closed RFA auction with one bid on it, plus the deadline the signing dates itself from.
async fn league_with_closed_rfa_auction(test_name: &str) -> Option<(TestLeague, i64, i64)> {
    let league = TestLeague::create(test_name, END_OF_SEASON_YEAR).await?;
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonFaAuctionStart,
            central("2025-09-20T12:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonFinalRosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;

    let rfa_player_id = league.add_veteran_player("Restricted Vet").await;
    let rfa_contract = league
        .add_unowned_contract(
            rfa_player_id,
            ContractKind::RestrictedFreeAgent,
            RFA_CARRY_SALARY,
        )
        .await;

    let auction = start_new_auction_for_nba_player(
        &rfa_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central("2025-09-05T12:00:00"),
        fbkl_entity::auction::AuctionKind::PreseasonVeteranAuction,
        RFA_CARRY_SALARY,
        &league.db,
    )
    .await
    .expect("start the RFA auction");

    let bidder = league.add_team_user(LeagueRole::TeamOwner).await;
    auction_queries::insert_auction_bid(auction.id, bidder.id, WINNING_BID, None, &league.db)
        .await
        .expect("insert the winning bid");

    Some((league, auction.id, rfa_player_id))
}

#[tokio::test]
async fn closing_an_rfa_auction_signs_nobody() {
    let Some((league, auction_id, rfa_player_id)) =
        league_with_closed_rfa_auction("rfa_auction_close").await
    else {
        return;
    };

    let closed_contract = end_veteran_auction(auction_id, None, &league.db)
        .await
        .expect("close the RFA auction");

    // Pooled contract comes back untouched: still restricted, unowned, and at its carry salary.
    assert_eq!(closed_contract.kind, ContractKind::RestrictedFreeAgent);
    assert_eq!(closed_contract.salary, RFA_CARRY_SALARY);
    assert_eq!(closed_contract.team_id, None);
    assert_eq!(closed_contract.player_id, Some(rfa_player_id));
    assert_eq!(
        auction_queries::find_auction_by_id(auction_id, &league.db)
            .await
            .expect("re-read the auction")
            .status,
        AuctionStatus::Closed
    );
}

#[tokio::test]
async fn an_unmatched_rfa_goes_to_the_winning_bidder_as_a_veteran() {
    let Some((league, auction_id, rfa_player_id)) =
        league_with_closed_rfa_auction("rfa_auction_no_match").await
    else {
        return;
    };
    end_veteran_auction(auction_id, None, &league.db)
        .await
        .expect("close the RFA auction");

    let signed_contract = resolve_rfa_auction_to_winning_bid(auction_id, None, None, &league.db)
        .await
        .expect("resolve the RFA auction");

    assert_eq!(signed_contract.kind, ContractKind::Veteran);
    assert_eq!(signed_contract.year_number, 1);
    assert_eq!(signed_contract.salary, WINNING_BID);
    assert_eq!(signed_contract.team_id, Some(league.team_id));
    assert_eq!(signed_contract.status, ContractStatus::Active);
    assert_eq!(signed_contract.player_id, Some(rfa_player_id));
    assert_eq!(
        auction_queries::find_auction_by_id(auction_id, &league.db)
            .await
            .expect("re-read the auction")
            .status,
        AuctionStatus::Completed
    );
}

/// The guard that keeps the no-match path off auctions the ordinary close already signed.
#[tokio::test]
async fn resolving_a_non_rfa_auction_is_rejected() {
    let Some(league) = TestLeague::create("rfa_auction_wrong_kind", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonFinalRosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;

    let player_id = league.add_veteran_player("Unrestricted Vet").await;
    let ufa_contract = league
        .add_unowned_contract(
            player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            RFA_CARRY_SALARY,
        )
        .await;
    let auction = start_new_auction_for_nba_player(
        &ufa_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central("2025-09-05T12:00:00"),
        fbkl_entity::auction::AuctionKind::PreseasonVeteranAuction,
        RFA_CARRY_SALARY,
        &league.db,
    )
    .await
    .expect("start the UFA auction");

    assert!(
        resolve_rfa_auction_to_winning_bid(auction.id, None, None, &league.db)
            .await
            .is_err()
    );
}
