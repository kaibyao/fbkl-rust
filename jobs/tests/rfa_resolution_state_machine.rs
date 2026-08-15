//! Cover for the RFA raise/match handshake (rules §15.3), which needs a database because every
//! step reads the resolution row seeded at the keeper deadline and rewrites the contract chain.
//!
//! The league here holds two teams: `owner_team_id` had the player at the keeper deadline and keeps
//! the discount right, and `league.team_id` places the winning bid and owns the draft picks that
//! could settle a decline.

use fbkl_entity::{
    contract::{ContractKind, ContractStatus},
    contract_queries,
    deadline::DeadlineKind,
    draft_pick_queries,
    rfa_resolution::{Model as RfaResolution, RfaResolutionStatus},
    rfa_resolution_queries,
    team_user::LeagueRole,
};
use fbkl_logic::{
    auction::{end_veteran_auction, start_new_auction_for_nba_player},
    deadline_processing::{
        RfaMatchDecision, UnbidRfaDecision, decline_to_raise, match_or_decline, raise_bid,
        resolve_unbid_rfa, seed_rfa_resolutions,
    },
};
use fbkl_test_support::{TestLeague, central};

const END_OF_SEASON_YEAR: i16 = 2026;
const RFA_CARRY_SALARY: i16 = 7;
const WINNING_BID: i16 = 19;
const AUCTION_START: &str = "2025-09-10T12:00:00";

struct RfaHandshake {
    league: TestLeague,
    rfa_resolution: RfaResolution,
    owner_team_id: i64,
    auction_id: i64,
}

/// A closed RFA auction whose resolution is waiting on the winner, plus `winner_pick_rounds` worth
/// of draft picks for the winning team.
async fn closed_rfa_auction(test_name: &str, winner_pick_rounds: &[i16]) -> Option<RfaHandshake> {
    let handshake = seeded_rfa_auction(test_name, winner_pick_rounds, true).await?;
    end_veteran_auction(handshake.auction_id, None, &handshake.league.db)
        .await
        .expect("close the RFA auction");
    Some(reread(handshake).await)
}

async fn seeded_rfa_auction(
    test_name: &str,
    winner_pick_rounds: &[i16],
    is_bid_on: bool,
) -> Option<RfaHandshake> {
    let league = TestLeague::create(test_name, END_OF_SEASON_YEAR).await?;
    for (kind, date_time) in [
        (
            DeadlineKind::PreseasonVeteranAuctionStart,
            "2025-09-01T12:00:00",
        ),
        (DeadlineKind::PreseasonFaAuctionStart, "2025-09-20T12:00:00"),
        (
            DeadlineKind::PreseasonFinalRosterLock,
            "2025-10-20T18:00:00",
        ),
    ] {
        league.add_deadline(kind, central(date_time)).await;
    }

    let owner_team_id = league.add_team("Keeper deadline owner").await;
    let rfa_player_id = league.add_veteran_player("Restricted Vet").await;
    let rfa_contract = league
        .add_owned_contract(
            rfa_player_id,
            ContractKind::RestrictedFreeAgent,
            RFA_CARRY_SALARY,
            owner_team_id,
        )
        .await;
    for round in winner_pick_rounds {
        league.add_draft_pick(*round, league.team_id).await;
    }

    seed_rfa_resolutions(league.league_id, END_OF_SEASON_YEAR, &league.db)
        .await
        .expect("seed RFA resolutions");

    let auction = start_new_auction_for_nba_player(
        &rfa_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central(AUCTION_START),
        fbkl_entity::auction::AuctionKind::PreseasonVeteranAuction,
        RFA_CARRY_SALARY,
        &league.db,
    )
    .await
    .expect("start the RFA auction");

    if is_bid_on {
        let bidder = league.add_team_user(LeagueRole::TeamOwner).await;
        fbkl_entity::auction_queries::insert_auction_bid(
            auction.id,
            bidder.id,
            WINNING_BID,
            None,
            &league.db,
        )
        .await
        .expect("insert the winning bid");
    }

    let rfa_resolution =
        rfa_resolution_queries::find_rfa_resolution_for_contract(rfa_contract.id, &league.db)
            .await
            .expect("read the seeded resolution")
            .expect("the keeper deadline seeds one");

    Some(RfaHandshake {
        league,
        rfa_resolution,
        owner_team_id,
        auction_id: auction.id,
    })
}

async fn reread(handshake: RfaHandshake) -> RfaHandshake {
    let rfa_resolution = rfa_resolution_queries::find_rfa_resolution_by_id(
        handshake.rfa_resolution.id,
        &handshake.league.db,
    )
    .await
    .expect("re-read the resolution");
    RfaHandshake {
        rfa_resolution,
        ..handshake
    }
}

#[tokio::test]
async fn closing_the_auction_opens_the_winners_raise_window() {
    let Some(handshake) = closed_rfa_auction("rfa_state_raise_window_opens", &[3]).await else {
        return;
    };
    let rfa_resolution = handshake.rfa_resolution;

    assert_eq!(rfa_resolution.status, RfaResolutionStatus::AwaitingRaise);
    assert_eq!(rfa_resolution.auction_id, Some(handshake.auction_id));
    assert_eq!(
        rfa_resolution.winning_team_id,
        Some(handshake.league.team_id)
    );
    assert_eq!(rfa_resolution.final_bid, Some(WINNING_BID));
    assert_eq!(rfa_resolution.effective_bid(), Some(WINNING_BID));
    // 48h from the auction's close, which is the start plus the 24h quiet window.
    assert_eq!(
        rfa_resolution.raise_deadline_at,
        Some(central("2025-09-13T12:00:00"))
    );
}

#[tokio::test]
async fn only_the_winning_bidder_may_raise_and_only_upward() {
    let Some(handshake) = closed_rfa_auction("rfa_state_raise_guards", &[3]).await else {
        return;
    };
    let db = &handshake.league.db;
    let rfa_resolution_id = handshake.rfa_resolution.id;
    let now = central("2025-09-11T12:00:00");

    assert!(
        raise_bid(rfa_resolution_id, handshake.owner_team_id, 25, now, db)
            .await
            .is_err(),
        "the original owner does not get to raise"
    );
    assert!(
        raise_bid(
            rfa_resolution_id,
            handshake.league.team_id,
            WINNING_BID,
            now,
            db
        )
        .await
        .is_err(),
        "a raise must beat the winning bid"
    );

    decline_to_raise(rfa_resolution_id, handshake.league.team_id, now, db)
        .await
        .expect("stand pat");
    assert!(
        raise_bid(rfa_resolution_id, handshake.league.team_id, 25, now, db)
            .await
            .is_err(),
        "the raise window is shut once the match window opens"
    );
}

#[tokio::test]
async fn a_raise_the_winner_cannot_compensate_for_is_rejected() {
    // A round 5 pick settles bids up to $11; raising to $42 owes a first-rounder (rules §15.2.1).
    let Some(handshake) = closed_rfa_auction("rfa_state_raise_unpayable", &[5]).await else {
        return;
    };
    assert!(
        raise_bid(
            handshake.rfa_resolution.id,
            handshake.league.team_id,
            42,
            central("2025-09-11T12:00:00"),
            &handshake.league.db
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn matching_re_signs_the_player_to_the_original_owner_at_a_discount() {
    let Some(handshake) = closed_rfa_auction("rfa_state_match", &[2, 3]).await else {
        return;
    };
    let db = &handshake.league.db;
    let rfa_resolution_id = handshake.rfa_resolution.id;

    raise_bid(
        rfa_resolution_id,
        handshake.league.team_id,
        24,
        central("2025-09-11T12:00:00"),
        db,
    )
    .await
    .expect("raise the winning bid");
    let matched = match_or_decline(
        rfa_resolution_id,
        handshake.owner_team_id,
        RfaMatchDecision::Match,
        None,
        central("2025-09-13T12:00:00"),
        db,
    )
    .await
    .expect("match the raised bid");

    assert_eq!(matched.status, RfaResolutionStatus::Resolved);
    assert_eq!(matched.raised_bid, Some(24));
    assert!(matched.resolved_at.is_some());

    let signed_contract =
        contract_queries::find_active_contracts_for_team(handshake.owner_team_id, db)
            .await
            .expect("read the owner's roster")
            .pop()
            .expect("the owner keeps the player");
    assert_eq!(signed_contract.kind, ContractKind::RookieExtension);
    assert_eq!(signed_contract.year_number, 4);
    // $24 less the uncapped 10% RFA discount, floored at the carry salary.
    assert_eq!(signed_contract.salary, 21);
    assert_eq!(signed_contract.status, ContractStatus::Active);
}

#[tokio::test]
async fn declining_signs_the_winner_and_hands_over_a_compensation_pick() {
    let Some(handshake) = closed_rfa_auction("rfa_state_decline", &[2, 3]).await else {
        return;
    };
    let db = &handshake.league.db;
    let rfa_resolution_id = handshake.rfa_resolution.id;

    decline_to_raise(
        rfa_resolution_id,
        handshake.league.team_id,
        central("2025-09-11T12:00:00"),
        db,
    )
    .await
    .expect("stand pat");
    let declined = match_or_decline(
        rfa_resolution_id,
        handshake.owner_team_id,
        RfaMatchDecision::Decline,
        None,
        central("2025-09-13T12:00:00"),
        db,
    )
    .await
    .expect("decline to match");
    assert_eq!(declined.status, RfaResolutionStatus::Declined);

    let winner_contracts =
        contract_queries::find_active_contracts_for_team(handshake.league.team_id, db)
            .await
            .expect("read the winner's roster");
    let signed_contract = winner_contracts
        .last()
        .expect("the winner signs the player");
    assert_eq!(signed_contract.kind, ContractKind::Veteran);
    assert_eq!(signed_contract.year_number, 1);
    assert_eq!(signed_contract.salary, WINNING_BID);

    let compensation =
        rfa_resolution_queries::find_rfa_compensation_pick_for_resolution(rfa_resolution_id, db)
            .await
            .expect("read the compensation row")
            .expect("a decline owes a pick");
    // $19 sits in the third-round tier, and no choice was named, so the cheapest one goes.
    assert_eq!(compensation.required_round, 3);
    let forfeited_pick = draft_pick_queries::find_draft_pick_by_id(
        compensation
            .forfeited_draft_pick_id
            .expect("the pick is chosen"),
        db,
    )
    .await
    .expect("read the forfeited pick");
    assert_eq!(forfeited_pick.round, 3);
    assert_eq!(
        forfeited_pick.current_owner_team_id,
        handshake.owner_team_id
    );
}

#[tokio::test]
async fn an_unbid_rfa_never_enters_the_raise_window() {
    let Some(handshake) = seeded_rfa_auction("rfa_state_unbid", &[], false).await else {
        return;
    };
    end_veteran_auction(handshake.auction_id, None, &handshake.league.db)
        .await
        .expect("close the unbid RFA auction");
    let handshake = reread(handshake).await;
    assert_eq!(
        handshake.rfa_resolution.status,
        RfaResolutionStatus::AwaitingAuction
    );

    let resigned = resolve_unbid_rfa(
        handshake.rfa_resolution.id,
        handshake.owner_team_id,
        UnbidRfaDecision::Resign,
        central("2025-09-13T12:00:00"),
        &handshake.league.db,
    )
    .await
    .expect("re-sign the unbid RFA");
    assert_eq!(resigned.status, RfaResolutionStatus::NoBidResigned);

    let signed_contract = contract_queries::find_active_contracts_for_team(
        handshake.owner_team_id,
        &handshake.league.db,
    )
    .await
    .expect("read the owner's roster")
    .pop()
    .expect("the owner keeps the player");
    assert_eq!(signed_contract.kind, ContractKind::RookieExtension);
    assert_eq!(signed_contract.year_number, 4);
    // Nobody bid, so the price is the standard 4th-year salary the RFA already carried.
    assert_eq!(signed_contract.salary, RFA_CARRY_SALARY);
}
