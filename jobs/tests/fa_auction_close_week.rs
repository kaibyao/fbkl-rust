//! What an in-season auction close does, and what it leaves for the owner (spec 08).
//!
//! Rules §8.3.6: the close records who won and signs nothing. The win becomes a contract when the
//! owner picks it up with the drops that make room for it, so the close writes no `team_update` and
//! joins no week; the lock it resolves is only the week the pickup will file under.

use chrono::Utc;
use fbkl_entity::{
    auction::{AuctionKind, AuctionStatus},
    auction_queries,
    contract::ContractKind,
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries,
    team_update::TeamUpdateStatus,
    team_update_queries::find_team_updates_by_team,
    team_user::LeagueRole,
};
use fbkl_logic::{
    auction::{find_auction_mode_deadlines, start_new_auction_for_nba_player},
    deadline_processing::{RosterRule, lock_rosters},
};
use fbkl_test_support::{TestLeague, central, days_ago, days_from_now};
use fbkl_transaction_processor::{
    ProcessOutcome, ProcessableEvent, ProcessableEventKind, process_event,
};

const END_OF_SEASON_YEAR: i16 = 2026;
const WINNING_BID: i16 = 5;
/// Rules §11.2: a roster carries at most 22 veteran or rookie-scale contracts.
const VET_OR_ROOKIE_LIMIT: usize = 22;
const PASSED_LOCK: &str = "2025-10-27T18:00:00";

#[tokio::test]
async fn a_close_records_the_win_without_signing_it() {
    let Some(league) = TestLeague::create("fa_auction_close_week", END_OF_SEASON_YEAR).await else {
        return;
    };
    league
        .add_deadline(DeadlineKind::Week1RosterLock, central(PASSED_LOCK))
        .await;
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(7))
        .await;
    // §4.2.3: an in-season lock resolves its cap against the auction period's end.
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_from_now(60))
        .await;

    let bidder = league.add_team_user(LeagueRole::TeamOwner).await;
    let player_id = league.add_veteran_player("Waiver Wire Vet").await;
    let pooled_contract = league
        .add_unowned_contract(
            player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            WINNING_BID,
        )
        .await;
    let auction = start_new_auction_for_nba_player(
        &pooled_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central(PASSED_LOCK),
        AuctionKind::InSeasonFreeAgent,
        WINNING_BID,
        &league.db,
    )
    .await
    .expect("start the in-season FA auction");
    auction_queries::insert_auction_bid(auction.id, bidder.id, WINNING_BID, None, &league.db)
        .await
        .expect("insert the winning bid");

    let outcome = process_event(
        &league.db,
        ProcessableEvent {
            league_id: league.league_id,
            end_of_season_year: END_OF_SEASON_YEAR,
            subject_id: auction.id,
            kind: ProcessableEventKind::FaAuctionClose,
        },
    )
    .await
    .expect("process the auction close");
    assert!(
        matches!(outcome, ProcessOutcome::Processed { .. }),
        "expected the close to run: {outcome:?}"
    );

    let closed_auction = auction_queries::find_auction_by_id(auction.id, &league.db)
        .await
        .expect("read the closed auction");
    assert_eq!(
        closed_auction.status,
        AuctionStatus::Won,
        "the close should record the win and leave the signing to the pickup"
    );

    let team_wins = auction_queries::find_won_auctions_for_team(
        league.team_id,
        league.league_id,
        END_OF_SEASON_YEAR,
        &league.db,
    )
    .await
    .expect("read the team's unsigned wins");
    assert_eq!(
        team_wins
            .iter()
            .map(|(won_auction, winning_bid)| (won_auction.id, winning_bid.bid_amount))
            .collect::<Vec<_>>(),
        vec![(auction.id, WINNING_BID)],
        "the win should be queryable by the team that won it"
    );

    for lock in [
        DeadlineKind::InSeasonRosterLock,
        DeadlineKind::Week1RosterLock,
    ] {
        let lock_id = deadline_id(&league, lock).await;
        let week = find_team_updates_by_team(league.team_id, None, Some(lock_id), &league.db)
            .await
            .expect("read the week's moves");
        assert!(
            week.is_empty(),
            "an unsigned win writes no move to {lock:?}: {week:?}"
        );
    }
}

/// Rules §8.1.3: pickups freeze at `FreeAgentAuctionEnd` "for the rest of the season (including
/// playoffs)". The season still has locks to fire here, so only the freeze can refuse the close.
#[tokio::test]
async fn a_close_after_the_free_agency_freeze_is_refused() {
    let Some(league) = TestLeague::create("fa_auction_close_frozen", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(7))
        .await;
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_ago(1))
        .await;

    let bidder = league.add_team_user(LeagueRole::TeamOwner).await;
    let player_id = league.add_veteran_player("Frozen Waiver Vet").await;
    let pooled_contract = league
        .add_unowned_contract(
            player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            WINNING_BID,
        )
        .await;
    // A row whose clock outlived the freeze, i.e. what a moved deadline or a stale auction leaves.
    let auction = start_new_auction_for_nba_player(
        &pooled_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        Utc::now().fixed_offset(),
        AuctionKind::InSeasonFreeAgent,
        WINNING_BID,
        &league.db,
    )
    .await
    .expect("start the in-season FA auction");
    auction_queries::insert_auction_bid(auction.id, bidder.id, WINNING_BID, None, &league.db)
        .await
        .expect("insert the winning bid");

    let outcome = process_event(
        &league.db,
        ProcessableEvent {
            league_id: league.league_id,
            end_of_season_year: END_OF_SEASON_YEAR,
            subject_id: auction.id,
            kind: ProcessableEventKind::FaAuctionClose,
        },
    )
    .await
    .expect("run the auction close");
    let ProcessOutcome::Failed { error, .. } = &outcome else {
        panic!("expected the frozen close to fail: {outcome:?}");
    };
    assert!(
        error.contains("in-season pickups froze at"),
        "the failure should name the free agency freeze: {error}"
    );

    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let upcoming_week = find_team_updates_by_team(league.team_id, None, Some(lock_id), &league.db)
        .await
        .expect("read the upcoming week's moves");
    assert!(
        upcoming_week.is_empty(),
        "a frozen pickup should not reach any week: {upcoming_week:?}"
    );
}

/// The §8.1.3 freeze bars pickups, not expiries, so a no-bid auction past it must still expire.
///
/// It used to be refused before the close outcome was computed, which left the row due for close
/// forever: every tick re-queued it and recorded another `Failed` run.
#[tokio::test]
async fn an_unbid_auction_past_the_free_agency_freeze_still_expires() {
    let Some(league) =
        TestLeague::create("fa_auction_close_unbid_frozen", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(7))
        .await;
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_ago(1))
        .await;

    let player_id = league.add_veteran_player("Unbid Frozen Vet").await;
    let pooled_contract = league
        .add_unowned_contract(
            player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            WINNING_BID,
        )
        .await;
    let auction = start_new_auction_for_nba_player(
        &pooled_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        Utc::now().fixed_offset(),
        AuctionKind::InSeasonFreeAgent,
        WINNING_BID,
        &league.db,
    )
    .await
    .expect("start the in-season FA auction");

    let outcome = process_event(
        &league.db,
        ProcessableEvent {
            league_id: league.league_id,
            end_of_season_year: END_OF_SEASON_YEAR,
            subject_id: auction.id,
            kind: ProcessableEventKind::FaAuctionClose,
        },
    )
    .await
    .expect("run the auction close");
    assert!(
        matches!(outcome, ProcessOutcome::Processed { .. }),
        "an unbid auction past the freeze should expire: {outcome:?}"
    );

    let closed_auction = auction_queries::find_auction_by_id(auction.id, &league.db)
        .await
        .expect("read the closed auction");
    assert_eq!(closed_auction.status, AuctionStatus::Expired);
}

/// An in-season auction is bounded by whichever lock comes next, and `Week1RosterLock` is one.
///
/// The lookup used to filter on `InSeasonRosterLock` alone, so an auction running in week 1 clamped
/// to the free agency freeze months away instead of Monday's lock.
#[tokio::test]
async fn the_in_season_hard_deadline_counts_the_week_1_lock_as_a_lock() {
    let Some(league) = TestLeague::create("fa_auction_week_1_lock", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    let week_1_lock = days_from_now(2);
    league
        .add_deadline(DeadlineKind::Week1RosterLock, week_1_lock)
        .await;
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(9))
        .await;
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_from_now(60))
        .await;

    let mode_deadlines = find_auction_mode_deadlines(
        AuctionKind::InSeasonFreeAgent,
        league.league_id,
        END_OF_SEASON_YEAR,
        Utc::now().fixed_offset(),
        &league.db,
    )
    .await
    .expect("find the auction's mode deadlines");

    assert_eq!(mode_deadlines.hard_deadline, week_1_lock);
}

async fn deadline_id(league: &TestLeague, kind: DeadlineKind) -> i64 {
    deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        kind,
        &league.db,
    )
    .await
    .expect("find deadline")
    .id
}

/// Weekly locks run through the playoff weeks to `SeasonEnd`, and rules §12.3 stop auctions once
/// the first playoff week starts, so a close with no lock left to fire is a broken season: it must
/// fail loudly instead of recording a win for a week that has already been locked.
#[tokio::test]
async fn an_auction_close_with_no_lock_left_fails_instead_of_joining_a_settled_week() {
    let Some(league) = TestLeague::create("fa_auction_close_no_lock", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(DeadlineKind::Week1RosterLock, central(PASSED_LOCK))
        .await;
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, central(PASSED_LOCK))
        .await;

    let bidder = league.add_team_user(LeagueRole::TeamOwner).await;
    let player_id = league.add_veteran_player("Late Waiver Vet").await;
    let pooled_contract = league
        .add_unowned_contract(
            player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            WINNING_BID,
        )
        .await;
    let auction = start_new_auction_for_nba_player(
        &pooled_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central(PASSED_LOCK),
        AuctionKind::InSeasonFreeAgent,
        WINNING_BID,
        &league.db,
    )
    .await
    .expect("start the in-season FA auction");
    auction_queries::insert_auction_bid(auction.id, bidder.id, WINNING_BID, None, &league.db)
        .await
        .expect("insert the winning bid");

    let outcome = process_event(
        &league.db,
        ProcessableEvent {
            league_id: league.league_id,
            end_of_season_year: END_OF_SEASON_YEAR,
            subject_id: auction.id,
            kind: ProcessableEventKind::FaAuctionClose,
        },
    )
    .await
    .expect("run the auction close");
    let ProcessOutcome::Failed { error, .. } = &outcome else {
        panic!("expected the close to fail: {outcome:?}");
    };
    assert!(
        error.contains("no roster lock is still to fire"),
        "the failure should name the missing lock: {error}"
    );

    let passed_lock_id = deadline_id(&league, DeadlineKind::Week1RosterLock).await;
    let settled_week =
        find_team_updates_by_team(league.team_id, None, Some(passed_lock_id), &league.db)
            .await
            .expect("read the settled week's moves");
    assert!(
        settled_week.is_empty(),
        "a settled week should not gain the signing: {settled_week:?}"
    );
}

/// Rules §8.3.5: a player won via auction must be picked up, so a win nobody picked up cannot
/// vanish at the lock. The lock signs it and lets the ordinary violation machinery record what it
/// costs (rules §13.1.2, §13.2).
#[tokio::test]
async fn a_win_nobody_picked_up_is_signed_at_the_lock() {
    let Some(league) = TestLeague::create("fa_auction_unpicked_win_lock", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(DeadlineKind::Week1RosterLock, central(PASSED_LOCK))
        .await;
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(7))
        .await;
    // §4.2.3: an in-season lock resolves its cap against the auction period's end.
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_from_now(60))
        .await;

    // A roster already at the §11.2 limit, so the unpicked win cannot fit.
    for index in 0..VET_OR_ROOKIE_LIMIT {
        let player_id = league
            .add_veteran_player(&format!("Holdover {index}"))
            .await;
        league
            .add_owned_contract(player_id, ContractKind::Veteran, 1, league.team_id)
            .await;
    }

    let bidder = league.add_team_user(LeagueRole::TeamOwner).await;
    let player_id = league.add_veteran_player("Unpicked Waiver Vet").await;
    let pooled_contract = league
        .add_unowned_contract(
            player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            WINNING_BID,
        )
        .await;
    let auction = start_new_auction_for_nba_player(
        &pooled_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central(PASSED_LOCK),
        AuctionKind::InSeasonFreeAgent,
        WINNING_BID,
        &league.db,
    )
    .await
    .expect("start the in-season FA auction");
    auction_queries::insert_auction_bid(auction.id, bidder.id, WINNING_BID, None, &league.db)
        .await
        .expect("insert the winning bid");
    process_event(
        &league.db,
        ProcessableEvent {
            league_id: league.league_id,
            end_of_season_year: END_OF_SEASON_YEAR,
            subject_id: auction.id,
            kind: ProcessableEventKind::FaAuctionClose,
        },
    )
    .await
    .expect("process the auction close");

    let in_season_lock = deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        DeadlineKind::InSeasonRosterLock,
        &league.db,
    )
    .await
    .expect("find the in-season lock");
    let violations = lock_rosters(&in_season_lock, &league.db)
        .await
        .expect("lock rosters");

    assert_eq!(
        violations
            .iter()
            .map(|violation| (violation.team_id, violation.rule))
            .collect::<Vec<_>>(),
        vec![(league.team_id, RosterRule::VeteranOrRookieLimit)],
        "the unpicked win puts the roster over the limit"
    );
    assert_eq!(
        auction_queries::find_auction_by_id(auction.id, &league.db)
            .await
            .expect("read the auction")
            .status,
        AuctionStatus::Completed,
        "the lock signs the win rather than losing it"
    );
    assert_eq!(
        contract_queries::find_active_contracts_for_team(league.team_id, &league.db)
            .await
            .expect("read the team's roster")
            .len(),
        VET_OR_ROOKIE_LIMIT + 1,
        "the signed win sits on the roster the commissioner has to rule on"
    );

    let week = find_team_updates_by_team(league.team_id, None, Some(in_season_lock.id), &league.db)
        .await
        .expect("read the week's moves");
    assert_eq!(
        week.iter()
            .map(|team_update| (team_update.status, team_update.transaction_number))
            .collect::<Vec<_>>(),
        vec![(TeamUpdateStatus::Pending, Some(0))],
        "the signing files as the week's first transaction and stays Pending for the illegal team"
    );
}
