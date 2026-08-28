//! Which week an in-season auction win lands in (spec 08).
//!
//! A week is the set of moves counting towards one roster lock, and a move made between two locks
//! belongs to the lock still to fire, not the one that already went. The auction close runs
//! server-side with no owner to name that deadline, so it has to look it up the same way.

use chrono::{Days, Utc};
use fbkl_entity::{
    auction::{AuctionKind, AuctionStatus},
    auction_queries,
    contract::ContractKind,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::prelude::DateTimeWithTimeZone,
    team_update_queries::find_team_updates_by_team,
    team_user::LeagueRole,
};
use fbkl_logic::auction::{find_auction_mode_deadlines, start_new_auction_for_nba_player};
use fbkl_test_support::{TestLeague, central};
use fbkl_transaction_processor::{
    ProcessOutcome, ProcessableEvent, ProcessableEventKind, process_event,
};

const END_OF_SEASON_YEAR: i16 = 2026;
const WINNING_BID: i16 = 5;
const PASSED_LOCK: &str = "2025-10-27T18:00:00";

#[tokio::test]
async fn an_auction_win_between_two_locks_is_filed_under_the_upcoming_lock() {
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

    let upcoming_lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let passed_lock_id = deadline_id(&league, DeadlineKind::Week1RosterLock).await;
    let upcoming_week =
        find_team_updates_by_team(league.team_id, None, Some(upcoming_lock_id), &league.db)
            .await
            .expect("read the upcoming week's moves");
    assert_eq!(
        upcoming_week.len(),
        1,
        "the auction win belongs to the week it landed in: {upcoming_week:?}"
    );
    let settled_week =
        find_team_updates_by_team(league.team_id, None, Some(passed_lock_id), &league.db)
            .await
            .expect("read the settled week's moves");
    assert!(
        settled_week.is_empty(),
        "the settled week should not gain a move: {settled_week:?}"
    );
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

fn days_from_now(days: u64) -> DateTimeWithTimeZone {
    Utc::now()
        .checked_add_days(Days::new(days))
        .expect("a date in the future")
        .fixed_offset()
}

fn days_ago(days: u64) -> DateTimeWithTimeZone {
    Utc::now()
        .checked_sub_days(Days::new(days))
        .expect("a date in the past")
        .fixed_offset()
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
/// fail loudly instead of dating the signing with a week that has already been locked.
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
