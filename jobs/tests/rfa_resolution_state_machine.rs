//! Cover for the RFA raise/match handshake (rules §15.3), which needs a database because every
//! step reads the resolution row seeded at the keeper deadline and rewrites the contract chain.
//!
//! The league here holds two teams: `owner_team_id` had the player at the keeper deadline and keeps
//! the discount right, and `league.team_id` places the winning bid and owns the draft picks that
//! could settle a decline.

use fbkl_entity::{
    auction::{AuctionKind, AuctionStatus},
    auction_queries,
    contract::{ContractKind, ContractStatus},
    contract_queries,
    deadline::DeadlineKind,
    draft_pick_queries,
    rfa_resolution::{Model as RfaResolution, RfaResolutionStatus},
    rfa_resolution_queries,
    team_user::LeagueRole,
};
use fbkl_jobs::{TickSummary, run_rfa_window_tick};
use fbkl_logic::{
    auction::{
        BidRejection, end_veteran_auction, get_or_create_player_contract_for_veteran_auction,
        place_auction_bid, start_new_auction_for_nba_player,
    },
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
    rfa_player_id: i64,
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
        AuctionKind::PreseasonVeteranAuction,
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
        rfa_player_id,
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

/// Rules §15.4.2: the discount belongs to the keeper-deadline owner, so a trade during the auction
/// hands over the player without handing over the discount.
#[tokio::test]
async fn matching_a_traded_rfa_still_re_signs_at_the_keeper_deadline_discount() {
    let Some(handshake) = seeded_rfa_auction("rfa_state_match_after_trade", &[3], true).await
    else {
        return;
    };
    let db = &handshake.league.db.clone();
    let acquiring_team_id = handshake.league.add_team("Acquired him mid-auction").await;

    // Moves the contract chain the way a processed trade does, minus the trade rows nothing here reads.
    let rfa_contract =
        contract_queries::find_contract_by_id(handshake.rfa_resolution.rfa_contract_id, db)
            .await
            .expect("read the designated RFA contract");
    let traded_contract =
        contract_queries::trade_contract_to_team(rfa_contract, acquiring_team_id, db)
            .await
            .expect("trade the RFA away from its keeper-deadline owner");
    assert_eq!(traded_contract.team_id, Some(acquiring_team_id));

    end_veteran_auction(handshake.auction_id, None, db)
        .await
        .expect("close the RFA auction");
    let handshake = reread(handshake).await;
    decline_to_raise(
        handshake.rfa_resolution.id,
        handshake.league.team_id,
        central("2025-09-11T12:00:00"),
        db,
    )
    .await
    .expect("stand pat");
    let matched = match_or_decline(
        handshake.rfa_resolution.id,
        handshake.owner_team_id,
        RfaMatchDecision::Match,
        None,
        central("2025-09-13T12:00:00"),
        db,
    )
    .await
    .expect("the keeper-deadline owner matches the bid");
    assert_eq!(matched.status, RfaResolutionStatus::Resolved);

    let signed_contract =
        contract_queries::find_active_contracts_for_team(handshake.owner_team_id, db)
            .await
            .expect("read the keeper-deadline owner's roster")
            .pop()
            .expect("matching brings the player back");
    assert_eq!(signed_contract.kind, ContractKind::RookieExtension);
    assert_eq!(signed_contract.year_number, 4);
    // $19 less the uncapped 10% RFA discount, the same price an untraded RFA would cost.
    assert_eq!(signed_contract.salary, 17);
    assert_eq!(signed_contract.status, ContractStatus::Active);
    assert_eq!(
        signed_contract.previous_contract_id,
        Some(traded_contract.id)
    );
    assert!(
        contract_queries::find_active_contracts_for_team(acquiring_team_id, db)
            .await
            .expect("read the acquiring team's roster")
            .is_empty(),
        "the team that acquired him loses him to the match"
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
    assert_eq!(
        auction_queries::find_auction_by_id(handshake.auction_id, db)
            .await
            .expect("re-read the auction")
            .status,
        AuctionStatus::Completed
    );

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
    assert!(
        auction_queries::find_winning_bids_for_team(
            handshake.league.team_id,
            handshake.league.league_id,
            END_OF_SEASON_YEAR,
            db
        )
        .await
        .expect("read the winner's commitments")
        .is_empty(),
        "the signed contract carries the salary, so the hold is counted once"
    );

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
    // Nobody bid, so the 10% discount comes off the carry salary with no floor (rules §15.3.5).
    assert_eq!(signed_contract.salary, 6, "$7 carry, $1 discount");
    assert_eq!(
        auction_queries::find_auction_by_id(handshake.auction_id, &handshake.league.db)
            .await
            .expect("re-read the RFA-week auction")
            .status,
        AuctionStatus::Completed,
        "the re-sign settles the auction the unbid RFA left Closed"
    );
}

#[tokio::test]
async fn an_unresolved_rfa_win_holds_the_winners_cap_space() {
    let Some(handshake) = closed_rfa_auction("rfa_state_cap_hold", &[3]).await else {
        return;
    };
    let league = &handshake.league;
    let db = &league.db;
    // $190 of roster salary plus the $19 hold leaves the winner $9 under the $200 preseason cap.
    let filler_player_id = league.add_veteran_player("Cap Filler").await;
    league
        .add_owned_contract(
            filler_player_id,
            ContractKind::RookieExtension,
            190,
            league.team_id,
        )
        .await;

    let other_player_id = league.add_veteran_player("Other Vet").await;
    let other_contract = league
        .add_unowned_contract(
            other_player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            1,
        )
        .await;
    let other_auction = start_new_auction_for_nba_player(
        &other_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central("2025-09-13T00:00:00"),
        AuctionKind::PreseasonVeteranAuction,
        1,
        db,
    )
    .await
    .expect("start a second auction");
    let bidder = league.add_team_user(LeagueRole::LeagueCommissioner).await;

    let rejection = place_auction_bid(
        other_auction.id,
        bidder.id,
        10,
        None,
        central("2025-09-13T06:00:00"),
        db,
    )
    .await
    .expect_err("the RFA hold leaves no room for a $10 bid");
    assert!(
        matches!(
            rejection.downcast_ref::<BidRejection>(),
            Some(BidRejection::InsufficientCap {
                committed_salary: 219,
                ..
            })
        ),
        "unexpected rejection: {rejection}"
    );

    decline_to_raise(
        handshake.rfa_resolution.id,
        league.team_id,
        central("2025-09-13T07:00:00"),
        db,
    )
    .await
    .expect("stand pat");
    match_or_decline(
        handshake.rfa_resolution.id,
        handshake.owner_team_id,
        RfaMatchDecision::Match,
        None,
        central("2025-09-13T08:00:00"),
        db,
    )
    .await
    .expect("match the bid");

    place_auction_bid(
        other_auction.id,
        bidder.id,
        10,
        None,
        central("2025-09-13T09:00:00"),
        db,
    )
    .await
    .expect("the match ends the hold and frees the winner's cap");
}

#[tokio::test]
async fn the_scheduler_tick_expires_both_handshake_windows() {
    let Some(handshake) = closed_rfa_auction("rfa_state_window_ticks", &[2, 3]).await else {
        return;
    };
    let db = &handshake.league.db.clone();
    let rfa_resolution_id = handshake.rfa_resolution.id;

    let raise_expiry = run_rfa_window_tick(db, central("2025-09-14T12:00:00"))
        .await
        .expect("expire the raise window");
    assert_eq!(raise_expiry.processed, 1);
    let handshake = reread(handshake).await;
    assert_eq!(
        handshake.rfa_resolution.status,
        RfaResolutionStatus::AwaitingMatch,
        "an unraised bid stands, which opens the original owner's window"
    );

    // The tick set the match deadline 48 hours out from the real clock, so backdate it.
    rfa_resolution_queries::open_rfa_match_window(
        rfa_resolution_id,
        None,
        central("2025-09-15T12:00:00"),
        db,
    )
    .await
    .expect("backdate the match deadline");

    let after_match_window = central("2025-09-16T12:00:00");
    let match_expiry = run_rfa_window_tick(db, after_match_window)
        .await
        .expect("expire the match window");
    assert_eq!(match_expiry.processed, 1);
    assert_eq!(
        run_rfa_window_tick(db, after_match_window)
            .await
            .expect("re-run the tick"),
        TickSummary::default(),
        "the resolution is settled, so a later tick has nothing to do"
    );

    let handshake = reread(handshake).await;
    assert_eq!(
        handshake.rfa_resolution.status,
        RfaResolutionStatus::Declined
    );
    let compensation =
        rfa_resolution_queries::find_rfa_compensation_pick_for_resolution(rfa_resolution_id, db)
            .await
            .expect("read the compensation row")
            .expect("an expired match window owes a pick");
    let forfeited_pick = draft_pick_queries::find_draft_pick_by_id(
        compensation
            .forfeited_draft_pick_id
            .expect("the pick is chosen"),
        db,
    )
    .await
    .expect("read the forfeited pick");
    assert_eq!(
        forfeited_pick.round, 3,
        "nobody named a pick, so the cheapest eligible one goes"
    );
    assert_eq!(
        forfeited_pick.current_owner_team_id,
        handshake.owner_team_id
    );
}

#[tokio::test]
async fn raising_then_declining_prices_the_signing_and_the_pick_off_the_raise() {
    let Some(handshake) = closed_rfa_auction("rfa_state_raise_then_decline", &[1, 3]).await else {
        return;
    };
    let db = &handshake.league.db;
    let rfa_resolution_id = handshake.rfa_resolution.id;

    raise_bid(
        rfa_resolution_id,
        handshake.league.team_id,
        30,
        central("2025-09-11T12:00:00"),
        db,
    )
    .await
    .expect("raise the winning bid");
    let declined = match_or_decline(
        rfa_resolution_id,
        handshake.owner_team_id,
        RfaMatchDecision::Decline,
        None,
        central("2025-09-13T12:00:00"),
        db,
    )
    .await
    .expect("decline the raised bid");
    assert_eq!(declined.status, RfaResolutionStatus::Declined);
    assert_eq!(declined.raised_bid, Some(30));

    let signed_contract =
        contract_queries::find_active_contracts_for_team(handshake.league.team_id, db)
            .await
            .expect("read the winner's roster")
            .pop()
            .expect("the winner signs the player");
    assert_eq!(signed_contract.kind, ContractKind::Veteran);
    assert_eq!(signed_contract.year_number, 1);
    assert_eq!(
        signed_contract.salary, 30,
        "the winner pays what it raised to"
    );

    let compensation =
        rfa_resolution_queries::find_rfa_compensation_pick_for_resolution(rfa_resolution_id, db)
            .await
            .expect("read the compensation row")
            .expect("a decline owes a pick");
    // $30 sits in the second-round tier, so the winner's 3rd is too cheap to settle it.
    assert_eq!(compensation.required_round, 2);
    let forfeited_pick = draft_pick_queries::find_draft_pick_by_id(
        compensation
            .forfeited_draft_pick_id
            .expect("the pick is chosen"),
        db,
    )
    .await
    .expect("read the forfeited pick");
    assert_eq!(forfeited_pick.round, 1);
    assert_eq!(
        forfeited_pick.current_owner_team_id,
        handshake.owner_team_id
    );
}

#[tokio::test]
async fn an_unbid_rfa_can_be_released_to_the_free_agent_auction() {
    let Some(handshake) = seeded_rfa_auction("rfa_state_unbid_released", &[], false).await else {
        return;
    };
    let db = &handshake.league.db;
    end_veteran_auction(handshake.auction_id, None, db)
        .await
        .expect("close the unbid RFA auction");

    let released = resolve_unbid_rfa(
        handshake.rfa_resolution.id,
        handshake.owner_team_id,
        UnbidRfaDecision::ReleaseToAuction,
        central("2025-09-13T12:00:00"),
        db,
    )
    .await
    .expect("release the unbid RFA");
    assert_eq!(released.status, RfaResolutionStatus::NoBidToAuction);
    assert!(released.resolved_at.is_some());

    assert!(
        contract_queries::find_active_contracts_for_team(handshake.owner_team_id, db)
            .await
            .expect("read the owner's roster")
            .is_empty(),
        "releasing the player takes him off the owner's roster"
    );
    let released_contract =
        contract_queries::find_contract_by_id(handshake.rfa_resolution.rfa_contract_id, db)
            .await
            .expect("read the designated RFA contract")
            .get_latest_in_chain(db)
            .await
            .expect("read the end of the contract chain");
    assert_eq!(released_contract.status, ContractStatus::Expired);
    assert_eq!(
        auction_queries::find_auction_by_id(handshake.auction_id, db)
            .await
            .expect("re-read the RFA-week auction")
            .status,
        AuctionStatus::Expired,
        "the release settles the auction the unbid RFA left Closed"
    );

    assert!(
        resolve_unbid_rfa(
            handshake.rfa_resolution.id,
            handshake.owner_team_id,
            UnbidRfaDecision::Resign,
            central("2025-09-14T12:00:00"),
            db,
        )
        .await
        .is_err(),
        "a settled resolution cannot be settled twice"
    );
}

/// Rules §15.3.5: a released RFA "is now on a new veteran contract, regardless of which owner
/// wins", so his old team may bid and wins him at the bid with no discount.
#[tokio::test]
async fn a_released_rfa_re_signs_as_a_plain_veteran_to_any_bidder() {
    let Some(handshake) = seeded_rfa_auction("rfa_state_released_reauction", &[], false).await
    else {
        return;
    };
    let league = &handshake.league;
    let db = &league.db;
    end_veteran_auction(handshake.auction_id, None, db)
        .await
        .expect("close the unbid RFA auction");
    resolve_unbid_rfa(
        handshake.rfa_resolution.id,
        handshake.owner_team_id,
        UnbidRfaDecision::ReleaseToAuction,
        central("2025-09-13T12:00:00"),
        db,
    )
    .await
    .expect("release the unbid RFA");

    let pooled_contract = get_or_create_player_contract_for_veteran_auction(
        league.league_id,
        END_OF_SEASON_YEAR,
        handshake.rfa_player_id,
        db,
    )
    .await
    .expect("pool the released player again");
    assert_eq!(
        pooled_contract.kind,
        ContractKind::FreeAgent,
        "the release leaves nothing restricted behind"
    );
    let new_auction = start_new_auction_for_nba_player(
        &pooled_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central("2025-09-14T12:00:00"),
        AuctionKind::PreseasonVeteranAuction,
        1,
        db,
    )
    .await
    .expect("start the released player's new auction");

    let former_owner = league
        .add_team_user_for_team(handshake.owner_team_id, LeagueRole::TeamOwner)
        .await;
    place_auction_bid(
        new_auction.id,
        former_owner.id,
        12,
        None,
        central("2025-09-14T13:00:00"),
        db,
    )
    .await
    .expect("the team that let him go may bid on him again");
    end_veteran_auction(new_auction.id, None, db)
        .await
        .expect("close the new auction");

    let signed_contract =
        contract_queries::find_active_contracts_for_team(handshake.owner_team_id, db)
            .await
            .expect("read the winner's roster")
            .pop()
            .expect("the winning bidder signs the player");
    assert_eq!(signed_contract.kind, ContractKind::Veteran);
    assert_eq!(signed_contract.year_number, 1);
    assert_eq!(signed_contract.salary, 12, "no discount on a fresh veteran");
    assert_eq!(
        auction_queries::find_auction_by_id(new_auction.id, db)
            .await
            .expect("re-read the new auction")
            .status,
        AuctionStatus::Completed,
        "the new auction signs on close instead of awaiting an RFA handshake"
    );
    assert!(
        rfa_resolution_queries::find_rfa_resolution_for_contract(pooled_contract.id, db)
            .await
            .expect("look for a second resolution")
            .is_none(),
        "the new auction writes no RFA resolution of its own"
    );
}

/// Rules §15.3.3: nobody may bid into a compensation tier he could not pay.
#[tokio::test]
async fn a_bid_owing_a_pick_the_bidder_lacks_is_rejected() {
    // A round 5 pick settles bids up to $11; $19 owes a third-rounder (rules §15.2.1).
    let Some(handshake) = seeded_rfa_auction("rfa_state_bid_unpayable", &[5], false).await else {
        return;
    };
    let db = &handshake.league.db;
    let bidder = handshake.league.add_team_user(LeagueRole::TeamOwner).await;

    let rejection = place_auction_bid(
        handshake.auction_id,
        bidder.id,
        WINNING_BID,
        None,
        central("2025-09-10T18:00:00"),
        db,
    )
    .await
    .expect_err("a fifth-rounder cannot settle a $19 bid");
    assert!(
        matches!(
            rejection.downcast_ref::<BidRejection>(),
            Some(BidRejection::MissingCompensationPick {
                required_round: 3,
                ..
            })
        ),
        "unexpected rejection: {rejection}"
    );

    place_auction_bid(
        handshake.auction_id,
        bidder.id,
        11,
        None,
        central("2025-09-10T18:00:00"),
        db,
    )
    .await
    .expect("$11 stays in the fifth-round tier");
}

#[tokio::test]
async fn a_bid_the_bidder_can_compensate_for_is_accepted() {
    let Some(handshake) = seeded_rfa_auction("rfa_state_bid_payable", &[3], false).await else {
        return;
    };
    let bidder = handshake.league.add_team_user(LeagueRole::TeamOwner).await;

    place_auction_bid(
        handshake.auction_id,
        bidder.id,
        WINNING_BID,
        None,
        central("2025-09-10T18:00:00"),
        &handshake.league.db,
    )
    .await
    .expect("a third-rounder settles a $19 bid");
}

/// Two live RFA debts need two picks: one third-rounder cannot be promised twice (rules §15.3.3).
#[tokio::test]
async fn two_live_rfa_bids_cannot_lean_on_the_same_pick() {
    let Some(handshake) = seeded_rfa_auction("rfa_state_bid_two_debts", &[3], false).await else {
        return;
    };
    let league = &handshake.league;
    let db = &league.db;

    let second_player_id = league.add_veteran_player("Second Restricted Vet").await;
    let second_rfa_contract = league
        .add_owned_contract(
            second_player_id,
            ContractKind::RestrictedFreeAgent,
            RFA_CARRY_SALARY,
            handshake.owner_team_id,
        )
        .await;
    seed_rfa_resolutions(league.league_id, END_OF_SEASON_YEAR, db)
        .await
        .expect("seed the second RFA's resolution");
    let second_auction = start_new_auction_for_nba_player(
        &second_rfa_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central(AUCTION_START),
        AuctionKind::PreseasonVeteranAuction,
        RFA_CARRY_SALARY,
        db,
    )
    .await
    .expect("start the second RFA auction");

    let bidder = league.add_team_user(LeagueRole::TeamOwner).await;
    place_auction_bid(
        handshake.auction_id,
        bidder.id,
        WINNING_BID,
        None,
        central("2025-09-10T18:00:00"),
        db,
    )
    .await
    .expect("the only third-rounder covers the first bid");

    let rejection = place_auction_bid(
        second_auction.id,
        bidder.id,
        WINNING_BID,
        None,
        central("2025-09-10T19:00:00"),
        db,
    )
    .await
    .expect_err("the same pick cannot back a second third-round debt");
    assert!(
        matches!(
            rejection.downcast_ref::<BidRejection>(),
            Some(BidRejection::MissingCompensationPick {
                required_round: 3,
                ..
            })
        ),
        "unexpected rejection: {rejection}"
    );

    // Even the mildest tier needs a pick of its own while the third-rounder backs the first bid.
    let rejection = place_auction_bid(
        second_auction.id,
        bidder.id,
        11,
        None,
        central("2025-09-10T19:00:00"),
        db,
    )
    .await
    .expect_err("the third-rounder is already promised elsewhere");
    assert!(
        matches!(
            rejection.downcast_ref::<BidRejection>(),
            Some(BidRejection::MissingCompensationPick {
                required_round: 5,
                ..
            })
        ),
        "unexpected rejection: {rejection}"
    );

    league.add_draft_pick(5, league.team_id).await;
    place_auction_bid(
        second_auction.id,
        bidder.id,
        11,
        None,
        central("2025-09-10T19:00:00"),
        db,
    )
    .await
    .expect("a second pick settles the second debt");
}
