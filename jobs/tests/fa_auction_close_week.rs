//! Which week an in-season auction win lands in (spec 08).
//!
//! A week is the set of moves counting towards one roster lock, and a move made between two locks
//! belongs to the lock still to fire, not the one that already went. The auction close runs
//! server-side with no owner to name that deadline, so it has to look it up the same way.

use chrono::{Days, Utc};
use fbkl_entity::{
    auction::AuctionKind, auction_queries, contract::ContractKind, deadline::DeadlineKind,
    deadline_queries, team_update_queries::find_team_updates_by_team, team_user::LeagueRole,
};
use fbkl_logic::auction::start_new_auction_for_nba_player;
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
    let days_from_now = |days: u64| {
        Utc::now()
            .checked_add_days(Days::new(days))
            .expect("a date in the future")
            .fixed_offset()
    };
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
