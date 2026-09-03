//! A veteran auction schedule row stays due forever once its release date passes, so every release
//! tick re-processes rows whose auction has already settled. These cover what that must not do:
//! re-open a settled player's auction, or let one bad row cost the league the rest of its tick.

use std::{
    io::Write,
    sync::{Arc, Mutex},
};

use chrono::{NaiveDate, TimeDelta};
use fbkl_entity::{
    auction::{self, AuctionStatus},
    auction_queries,
    deadline::DeadlineKind,
    sea_orm::{ConnectionTrait, EntityTrait},
    team_user::LeagueRole,
};
use fbkl_jobs::run_veteran_auction_release_tick;
use fbkl_logic::auction::end_veteran_auction;
use fbkl_test_support::{TestLeague, central, now_storable};

const END_OF_SEASON_YEAR: i16 = 2026;
const TIER_MIN_BID_AMOUNTS: [i16; 4] = [20, 15, 10, 5];
const TOP_TIER_INDEX: i16 = 0;

/// A settled auction's contract chain no longer points at the pooled contract, which is exactly the
/// state the release tick used to choke on.
#[tokio::test]
async fn a_completed_auction_does_not_stall_the_rest_of_the_league_tick() {
    let Some(league) = seeded_league("vet_auction_completed_row").await else {
        return;
    };
    let team_user_id = league.add_team_user(LeagueRole::TeamOwner).await.id;
    let signed_player_id = league.add_veteran_player("Signed Vet").await;
    let unbid_player_id = league.add_veteran_player("Unbid Vet").await;
    let later_player_id = league.add_veteran_player("Later Vet").await;
    league
        .add_schedule_row(signed_player_id, release_day(1), TOP_TIER_INDEX)
        .await;
    league
        .add_schedule_row(unbid_player_id, release_day(1), TOP_TIER_INDEX)
        .await;
    league
        .add_schedule_row(later_player_id, release_day(2), TOP_TIER_INDEX)
        .await;

    let day_one_summary =
        run_veteran_auction_release_tick(&league.db, central("2025-09-01T12:00:00"))
            .await
            .expect("run the day-one release tick");
    assert_eq!((day_one_summary.errors, day_one_summary.failed), (0, 0));

    let signed_auction = auction_for(&league, signed_player_id).await;
    auction_queries::insert_auction_bid(
        signed_auction.id,
        team_user_id,
        TIER_MIN_BID_AMOUNTS[0],
        None,
        &league.db,
    )
    .await
    .expect("place the winning bid");
    end_veteran_auction(signed_auction.id, None, &league.db)
        .await
        .expect("complete the auction");
    backdate_to_yesterday(&league, auction_for(&league, unbid_player_id).await.id).await;

    let day_two_summary =
        run_veteran_auction_release_tick(&league.db, central("2025-09-02T12:00:00"))
            .await
            .expect("run the day-two release tick");
    assert_eq!((day_two_summary.errors, day_two_summary.failed), (0, 0));
    assert_eq!(
        auction_for(&league, signed_player_id).await.status,
        AuctionStatus::Completed
    );
    assert_eq!(
        auction_for(&league, later_player_id).await.status,
        AuctionStatus::Open
    );
    // The slide runs after every row, so a slid tier proves the tick did not abort on the completed one.
    assert_eq!(
        auction_for(&league, unbid_player_id)
            .await
            .minimum_bid_amount,
        TIER_MIN_BID_AMOUNTS[1]
    );
}

/// Expiry leaves the player with no active contract at all, which used to look like "never opened".
#[tokio::test]
async fn an_expired_auction_is_not_reopened_on_the_next_tick() {
    let Some(league) = seeded_league("vet_auction_expired_row").await else {
        return;
    };
    let player_id = league.add_veteran_player("Nobody Bid").await;
    league
        .add_schedule_row(player_id, release_day(1), TOP_TIER_INDEX)
        .await;

    run_veteran_auction_release_tick(&league.db, central("2025-09-01T12:00:00"))
        .await
        .expect("run the day-one release tick");
    let opened_auction = auction_for(&league, player_id).await;
    end_veteran_auction(opened_auction.id, None, &league.db)
        .await
        .expect("expire the unbid auction");

    let day_two_summary =
        run_veteran_auction_release_tick(&league.db, central("2025-09-02T12:00:00"))
            .await
            .expect("run the day-two release tick");
    assert_eq!((day_two_summary.errors, day_two_summary.failed), (0, 0));

    let all_auctions = auction::Entity::find()
        .all(&league.db)
        .await
        .expect("read every auction");
    assert_eq!(all_auctions.len(), 1);
    assert_eq!(all_auctions[0].id, opened_auction.id);
    assert_eq!(all_auctions[0].status, AuctionStatus::Expired);
}

#[tokio::test]
async fn a_failing_schedule_row_is_skipped_rather_than_fatal() {
    let Some(league) = seeded_league("vet_auction_failing_row").await else {
        return;
    };
    let broken_player_id = league.add_veteran_player("Bad Tier").await;
    let good_player_id = league.add_veteran_player("Good Tier").await;
    // No tier 99 is configured, so opening this row errors out.
    league
        .add_schedule_row(broken_player_id, release_day(1), 99)
        .await;
    league
        .add_schedule_row(good_player_id, release_day(1), TOP_TIER_INDEX)
        .await;

    let captured_logs = CapturedLogs::start();
    let summary = run_veteran_auction_release_tick(&league.db, central("2025-09-01T12:00:00"))
        .await
        .expect("run the release tick");
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.failed, 1);
    assert_eq!(
        auction_for(&league, good_player_id).await.status,
        AuctionStatus::Open
    );
    assert!(
        league
            .find_veteran_auction(broken_player_id)
            .await
            .is_none()
    );
    // Swallowing the row is only acceptable because the commissioner can still find it in the logs.
    let logged = captured_logs.text();
    assert!(
        logged.contains("Failed to open scheduled veteran auction"),
        "the skipped row was not logged: {logged}"
    );
    assert!(
        logged.contains(&format!("player id = {broken_player_id}")),
        "the log does not name the skipped player: {logged}"
    );
}

/// §6.4.4's crunch sweep runs after every row, so a row that cannot open must not cost the league's
/// live auctions their shortened reprieve.
#[tokio::test]
async fn a_failing_row_still_leaves_the_crunch_sweep_to_run() {
    // Bid timestamps come from the database clock, so this league's deadlines follow wall-clock now.
    let now = now_storable();
    let hard_deadline = now + TimeDelta::hours(6);
    let Some(league) =
        TestLeague::create("vet_auction_crunch_after_failure", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            now - TimeDelta::days(1),
        )
        .await;
    league
        .add_deadline(DeadlineKind::PreseasonFinalRosterLock, hard_deadline)
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;

    let bid_player_id = league.add_veteran_player("Contested Vet").await;
    let broken_player_id = league.add_veteran_player("Bad Tier").await;
    league
        .add_schedule_row(bid_player_id, now.date_naive(), TOP_TIER_INDEX)
        .await;
    league
        .add_schedule_row(broken_player_id, now.date_naive(), 99)
        .await;

    run_veteran_auction_release_tick(&league.db, now)
        .await
        .expect("run the opening tick");
    let opened_auction = auction_for(&league, bid_player_id).await;
    assert_eq!(opened_auction.close_at_timestamp, hard_deadline);
    auction_queries::insert_auction_bid(
        opened_auction.id,
        league.add_team_user(LeagueRole::TeamOwner).await.id,
        TIER_MIN_BID_AMOUNTS[0],
        None,
        &league.db,
    )
    .await
    .expect("place a bid inside the crunch window");

    let summary = run_veteran_auction_release_tick(&league.db, now)
        .await
        .expect("run the crunch-window tick");
    assert_eq!((summary.errors, summary.failed), (0, 1));
    // One hour of reprieve, not the usual 24, and well short of the hard deadline it opened on.
    let crunched_auction = auction_for(&league, bid_player_id).await;
    assert!(
        crunched_auction.close_at_timestamp < opened_auction.close_at_timestamp,
        "the crunch sweep did not shorten the auction"
    );
    assert!(
        crunched_auction.close_at_timestamp <= now + TimeDelta::hours(1) + TimeDelta::minutes(5)
    );
}

async fn seeded_league(test_name: &str) -> Option<TestLeague> {
    let league = TestLeague::create(test_name, END_OF_SEASON_YEAR).await?;
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    // Signing a winning bid stamps its league event with the FA auction start deadline.
    league
        .add_deadline(
            DeadlineKind::PreseasonFaAuctionStart,
            central("2025-10-01T12:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonFinalRosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;
    Some(league)
}

const fn release_day(day_of_september: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 9, day_of_september).expect("valid release date")
}

async fn auction_for(league: &TestLeague, player_id: i64) -> auction::Model {
    league
        .find_veteran_auction(player_id)
        .await
        .unwrap_or_else(|| panic!("player {player_id} has no auction"))
}

/// Everything logged on this thread while it is alive, so a test can assert on a swallowed error.
struct CapturedLogs {
    buffer: LogBuffer,
    _subscriber_guard: tracing::subscriber::DefaultGuard,
}

impl CapturedLogs {
    fn start() -> Self {
        let buffer = LogBuffer(Arc::new(Mutex::new(Vec::new())));
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        Self {
            buffer,
            _subscriber_guard: tracing::subscriber::set_default(subscriber),
        }
    }

    fn text(&self) -> String {
        String::from_utf8(self.buffer.0.lock().expect("read captured logs").clone())
            .expect("logs are utf-8")
    }
}

#[derive(Clone)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("write captured logs")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Timestamp columns default to wall-clock now, so an auction opened at a historical test date is
/// still "touched today" and the tier slide skips it until it is aged by hand.
async fn backdate_to_yesterday(league: &TestLeague, auction_id: i64) {
    league
        .db
        .execute_unprepared(&format!(
            "UPDATE auction SET updated_at = '2025-08-31T12:00:00-06:00' WHERE id = {auction_id}"
        ))
        .await
        .expect("backdate auction");
}
