//! Two submissions for one team that overlap in time must stay two transactions (rules §13.1.4).
//!
//! A transaction is delimited by the newest move id its team's week already held, so an owner who
//! double-submits (two tabs, or a client retry) can have both requests read the same starting
//! point. Without a lock the second request numbers the first one's already-judged moves into its
//! own transaction, and both batches are then judged as one unit.

use chrono::Utc;
use fbkl_entity::{
    contract::{self, ContractKind},
    deadline::{self, DeadlineKind},
    deadline_queries,
    sea_orm::{DatabaseTransaction, TransactionTrait},
    team_update_queries::{self, find_transaction_start},
};
use fbkl_logic::{drop_contract::drop_contract_from_team, roster::file_and_validate_transaction};
use fbkl_test_support::{TestLeague, central, days_from_now};

const END_OF_SEASON_YEAR: i16 = 2026;

#[tokio::test]
async fn two_overlapping_submissions_for_one_team_stay_two_transactions() {
    let Some(league) = TestLeague::create("submit_transaction_race", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::Week1RosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::FreeAgentAuctionEnd,
            central("2026-03-01T18:00:00"),
        )
        .await;
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(3))
        .await;
    let lock = deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        DeadlineKind::InSeasonRosterLock,
        &league.db,
    )
    .await
    .expect("find the in-season roster lock");

    let roster = add_roster_contracts(&league, 4).await;

    let first = league.db.begin().await.expect("start the first submission");
    let second = league
        .db
        .begin()
        .await
        .expect("start the second submission");
    let (first_outcome, second_outcome) = tokio::join!(
        drop_and_commit(league.team_id, &roster[0], &lock, first),
        drop_and_commit(league.team_id, &roster[1], &lock, second),
    );
    first_outcome.expect("the first submission is legal");
    second_outcome.expect("the second submission is legal");

    // Two drops, each its own transaction: the second must not renumber the first's move.
    let week_moves = team_update_queries::find_team_updates_by_team(
        league.team_id,
        None,
        Some(lock.id),
        &league.db,
    )
    .await
    .expect("read the team's week");
    let mut numbers: Vec<Option<i16>> = week_moves
        .iter()
        .map(|team_update| team_update.transaction_number)
        .collect();
    numbers.sort_unstable();
    assert_eq!(
        numbers,
        vec![Some(0), Some(1)],
        "each submission takes a transaction number of its own: {week_moves:?}"
    );
}

/// One writer's submission: a lone drop, filed and judged as its own transaction.
async fn drop_and_commit(
    team_id: i64,
    contract_model: &contract::Model,
    lock: &deadline::Model,
    db_txn: DatabaseTransaction,
) -> Result<(), String> {
    let transaction_start = find_transaction_start(team_id, lock.id, &db_txn)
        .await
        .map_err(|error| error.to_string())?;
    drop_contract_from_team(contract_model.clone(), lock, &db_txn)
        .await
        .map_err(|error| error.to_string())?;
    file_and_validate_transaction(
        team_id,
        lock,
        &transaction_start,
        &Utc::now().fixed_offset(),
        &db_txn,
    )
    .await
    .map_err(|error| error.to_string())?;
    db_txn.commit().await.map_err(|error| error.to_string())
}

/// `count` $1 contracts owned by the league's team, i.e. roster filler that never breaks the cap.
async fn add_roster_contracts(league: &TestLeague, count: usize) -> Vec<contract::Model> {
    let mut contracts = Vec::with_capacity(count);
    for index in 0..count {
        let player_id = league.add_veteran_player(&format!("Filler {index}")).await;
        contracts.push(
            league
                .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
                .await,
        );
    }
    contracts
}
