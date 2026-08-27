//! Owner roster moves during the playoff weeks, and after the season's last lock.
//!
//! Rules decision: in-roster moves (IR, activations) stay legal through the playoff weeks, so a
//! league's weekly locks run to `SeasonEnd` and a move made in a playoff week counts towards that
//! week's lock like any other. Trades and auctions stop at the first playoff week, which is a
//! separate gate.
//!
//! Once a season truly has no lock left to fire, the league's deadlines are incomplete: the refusal
//! has to say so rather than read as "that deadline is not the upcoming one".

use std::sync::Arc;

use async_graphql::{Request, Value};
use chrono::{Days, Utc};
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::{ActiveValue, prelude::DateTimeWithTimeZone},
    team_update::{self, TeamUpdateData, TeamUpdateStatus},
    team_update_queries,
    team_user::LeagueRole,
    transaction::{self, TransactionKind},
    transaction_queries,
};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::TestLeague;
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;

#[tokio::test]
async fn an_ir_move_in_a_playoff_week_counts_towards_that_weeks_lock() {
    let Some(league) = TestLeague::create("playoff_week_ir_move", END_OF_SEASON_YEAR).await else {
        return;
    };
    let playoff_start = add_season_past(&league).await;
    // The lock at the start of the next playoff week, i.e. the one this move is judged at.
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(2))
        .await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let holdover = add_roster_contract(&league, "Playoff Holdover").await;
    // A settled week that committed the contract to the active roster (rules §10.3.1).
    record_committed_roster(&league, playoff_start, holdover.id).await;

    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    move_to_ir(&schema, holdover.id, lock_id, &session)
        .await
        .expect("an IR move during a playoff week is legal");
    assert_eq!(
        ir_contract_count(&league).await,
        1,
        "the contract should now be on IR"
    );
    let week = team_update_queries::find_team_updates_by_team(
        league.team_id,
        None,
        Some(lock_id),
        &league.db,
    )
    .await
    .expect("read the playoff week's moves");
    assert_eq!(
        week.len(),
        1,
        "the move belongs to the playoff week's lock: {week:?}"
    );
}

#[tokio::test]
async fn a_season_with_no_lock_left_says_the_locks_are_missing() {
    let Some(league) = TestLeague::create("playoff_week_no_lock_left", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    let playoff_start = add_season_past(&league).await;
    // The season's last lock has already fired, so nothing is left to judge a move at.
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_ago(1))
        .await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let holdover = add_roster_contract(&league, "Stranded Holdover").await;
    record_committed_roster(&league, playoff_start, holdover.id).await;

    let passed_lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let (code, message) = move_to_ir(&schema, holdover.id, passed_lock_id, &session)
        .await
        .expect_err("a settled week cannot take a new move");
    assert_eq!(code.as_deref(), Some("BAD_REQUEST"));
    assert!(
        message.contains("no roster lock still to fire"),
        "the owner should be told the season's locks are missing: {message}"
    );
    assert_eq!(
        ir_contract_count(&league).await,
        0,
        "the refused move should not have been applied"
    );
}

/// The deadlines of a season already in its playoff weeks, returning the playoff start's id.
///
/// `FreeAgentAuctionEnd` is required: an in-season lock resolves its cap against it (rules §4.2.3).
async fn add_season_past(league: &TestLeague) -> i64 {
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_ago(30))
        .await;
    league
        .add_deadline(DeadlineKind::TradeDeadlineAndPlayoffStart, days_ago(10))
        .await;
    league
        .add_deadline(DeadlineKind::SeasonEnd, days_from_now(14))
        .await;

    deadline_id(league, DeadlineKind::TradeDeadlineAndPlayoffStart).await
}

/// One $1 contract owned by the league's team, i.e. roster filler that never breaks the cap.
async fn add_roster_contract(league: &TestLeague, player_name: &str) -> contract::Model {
    let player_id = league.add_veteran_player(player_name).await;
    league
        .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
        .await
}

/// A Done `team_update` from a settled week whose committed roster holds `contract_id`.
///
/// It records no asset change, so it reads as a week the contract sat through rather than the add
/// that brought it in (rules §10.3.1).
async fn record_committed_roster(league: &TestLeague, deadline_id: i64, contract_id: i64) {
    let transaction_model = transaction_queries::insert_transaction(
        transaction::ActiveModel {
            end_of_season_year: ActiveValue::Set(END_OF_SEASON_YEAR),
            kind: ActiveValue::Set(TransactionKind::AuctionDone),
            league_id: ActiveValue::Set(league.league_id),
            deadline_id: ActiveValue::Set(deadline_id),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert transaction");

    let data = TeamUpdateData::from_assets(vec![contract_id], vec![], 0, 0, 0, 0)
        .to_json()
        .expect("team update data as json");
    team_update_queries::insert_team_update(
        team_update::ActiveModel {
            data: ActiveValue::Set(data),
            effective_date: ActiveValue::Set(days_ago(10).date_naive()),
            status: ActiveValue::Set(TeamUpdateStatus::Done),
            team_id: ActiveValue::Set(league.team_id),
            transaction_id: ActiveValue::Set(Some(transaction_model.id)),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert team update");
}

/// Runs `moveContractToIr`, returning the error code and message when it is refused.
async fn move_to_ir(
    schema: &AppSchema,
    contract_id: i64,
    deadline_id: i64,
    session: &Session,
) -> Result<(), (Option<String>, String)> {
    let mutation = format!(
        "mutation {{ moveContractToIr(contractId: {contract_id}, deadlineId: {deadline_id}) {{ id }} }}"
    );
    let response = schema
        .execute(Request::new(mutation).data(session.clone()))
        .await;
    if let Some(error) = response.errors.first() {
        let code = error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .map(|code| code.to_string().trim_matches('"').to_owned());
        return Err((code, error.message.clone()));
    }
    assert!(
        matches!(response.data, Value::Object(_)),
        "a successful move returns the contract: {:?}",
        response.data
    );

    Ok(())
}

/// How many of the team's active contracts sit on IR.
async fn ir_contract_count(league: &TestLeague) -> usize {
    contract_queries::find_active_contracts_for_team(league.team_id, &league.db)
        .await
        .expect("load the team's contracts")
        .iter()
        .filter(|contract_model| contract_model.is_ir)
        .count()
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

/// A logged-in session for one user in one league, i.e. what the session layer would have built.
async fn session_for(user_id: i64, league_id: i64) -> Session {
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    session
        .insert("user_id", user_id)
        .await
        .expect("set the session user");
    session
        .insert("selected_league_id", league_id)
        .await
        .expect("set the session league");
    session
}
