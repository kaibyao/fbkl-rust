//! The `PreseasonVeteranAuctionStart` deadline is what puts the veteran auction in motion (§6.3.1):
//! processing it assembles the pool, and the release tick then opens each row on its date.

mod common;

use chrono::Days;
use common::{TestLeague, central};
use fbkl_entity::{
    auction::AuctionStatus,
    auction_schedule_queries,
    contract::ContractKind,
    deadline::{self, DeadlineKind},
    deadline_queries,
};
use fbkl_jobs::run_veteran_auction_release_tick;
use fbkl_logic::auction::assemble_veteran_auction_pool;
use fbkl_transaction_processor::{ProcessOutcome, process_deadline};

const END_OF_SEASON_YEAR: i16 = 2026;
const TIER_MIN_BID_AMOUNTS: [i16; 4] = [20, 15, 10, 5];
const AUCTION_START: &str = "2025-09-01T12:00:00";

#[tokio::test]
async fn the_start_deadline_assembles_the_pool_and_the_tick_opens_rfa_week() {
    let Some(league) =
        TestLeague::create("veteran_auction_start_deadline", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central(AUCTION_START),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonFinalRosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;
    league.add_min_bid_tiers(&TIER_MIN_BID_AMOUNTS).await;

    let rfa_player_id = league.add_veteran_player("Restricted Vet").await;
    league
        .add_unowned_contract(rfa_player_id, ContractKind::RestrictedFreeAgent, 7)
        .await;
    let first_other_player_id = league.add_veteran_player("Best Vet").await;
    let second_other_player_id = league.add_veteran_player("Second Vet").await;
    league
        .add_ranked_players(&[first_other_player_id, second_other_player_id])
        .await;

    let outcome = process_deadline(&league.db, &start_deadline(&league).await)
        .await
        .expect("process the veteran auction start deadline");
    assert!(matches!(outcome, ProcessOutcome::Processed { .. }));

    // §6.3.1: RFAs release on the start date, everyone else only after RFA week.
    let schedule_rows = season_schedule_rows(&league).await;
    assert_eq!(schedule_rows.len(), 3);
    let rfa_row = schedule_rows
        .iter()
        .find(|row| row.player_id == rfa_player_id)
        .expect("the RFA is scheduled");
    assert!(rfa_row.is_rfa_week);
    assert_eq!(
        rfa_row.scheduled_release_date,
        central(AUCTION_START).date_naive()
    );
    // RFA week is a full seven days, so the first non-RFA release is exactly a week out.
    let first_other_release_date = rfa_row.scheduled_release_date + Days::new(7);
    for other_row in schedule_rows
        .iter()
        .filter(|row| row.player_id != rfa_player_id)
    {
        assert!(!other_row.is_rfa_week);
        assert_eq!(other_row.scheduled_release_date, first_other_release_date);
    }

    // A retried deadline re-runs the handler, so the pool guard - not the job_run - is what must hold.
    assert!(matches!(
        process_deadline(&league.db, &start_deadline(&league).await).await,
        Ok(ProcessOutcome::AlreadyProcessed)
    ));
    assemble_veteran_auction_pool(league.league_id, END_OF_SEASON_YEAR, &league.db)
        .await
        .expect("re-assemble the veteran auction pool");
    assert_eq!(season_schedule_rows(&league).await.len(), 3);

    let summary = run_veteran_auction_release_tick(&league.db, central("2025-09-01T13:00:00"))
        .await
        .expect("run the release tick");
    assert_eq!((summary.errors, summary.failed), (0, 0));
    assert_eq!(
        league
            .find_veteran_auction(rfa_player_id)
            .await
            .expect("the RFA auction opened on day one")
            .status,
        AuctionStatus::Open
    );
    assert!(
        league
            .find_veteran_auction(first_other_player_id)
            .await
            .is_none()
    );
    assert!(
        league
            .find_veteran_auction(second_other_player_id)
            .await
            .is_none()
    );

    let week_later_summary =
        run_veteran_auction_release_tick(&league.db, central("2025-09-08T13:00:00"))
            .await
            .expect("run the release tick a week later");
    assert_eq!(
        (week_later_summary.errors, week_later_summary.failed),
        (0, 0)
    );
    for player_id in [first_other_player_id, second_other_player_id] {
        assert_eq!(
            league
                .find_veteran_auction(player_id)
                .await
                .expect("the non-RFA auction opened once RFA week ended")
                .status,
            AuctionStatus::Open
        );
    }
}

async fn start_deadline(league: &TestLeague) -> deadline::Model {
    deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        DeadlineKind::PreseasonVeteranAuctionStart,
        &league.db,
    )
    .await
    .expect("find the veteran auction start deadline")
}

async fn season_schedule_rows(league: &TestLeague) -> Vec<fbkl_entity::auction_schedule::Model> {
    auction_schedule_queries::find_auction_schedule_rows_for_season(
        league.league_id,
        END_OF_SEASON_YEAR,
        &league.db,
    )
    .await
    .expect("read the season's schedule rows")
}
