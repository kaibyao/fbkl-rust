//! The commissioner reads a lock's recorded roster failures through the API (rules §13.1.2, §13.2).
//!
//! Roster lock writes the rows (covered in `jobs/tests/weekly_moves_and_roster_legalization.rs`);
//! this file seeds them directly and pins what the query gives back, and to whom.

use std::sync::Arc;

use async_graphql::{Name, Request, Response, Value};
use fbkl_entity::{
    deadline::{self, DeadlineKind},
    deadline_queries,
    roster_lock_violation::RosterRule,
    roster_lock_violation_queries::{TeamRosterViolation, replace_violations_for_deadline},
    team_user::LeagueRole,
};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;
const QUERY: &str = "query { rosterLockViolations { deadlineId teamId rule message } }";

#[tokio::test]
async fn the_commissioner_reads_the_locks_recorded_violations() {
    let Some(league) = TestLeague::create("roster_lock_violations_query", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    let week_1_lock = seed_recorded_violation(&league).await;
    let commissioner = league.add_team_user(LeagueRole::LeagueCommissioner).await;

    let response = run_query(&league, commissioner.user_id).await;

    assert!(
        response.errors.is_empty(),
        "the commissioner may read them: {:?}",
        response.errors
    );
    let Value::List(violations) = field(&response.data, "rosterLockViolations") else {
        panic!("expected a list of violations");
    };
    assert_eq!(violations.len(), 1);
    assert_eq!(
        field(&violations[0], "deadlineId"),
        &Value::Number(week_1_lock.id.into())
    );
    assert_eq!(
        field(&violations[0], "teamId"),
        &Value::Number(league.team_id.into())
    );
    assert_eq!(
        field(&violations[0], "rule"),
        &Value::Enum(Name::new("VETERAN_OR_ROOKIE_LIMIT"))
    );
    assert_eq!(
        field(&violations[0], "message"),
        &Value::String("23 of a possible 22 veteran or rookie contracts".to_owned())
    );
}

#[tokio::test]
async fn an_owner_cannot_read_the_leagues_violations() {
    let Some(league) =
        TestLeague::create("roster_lock_violations_query_owner", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    seed_recorded_violation(&league).await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    let response = run_query(&league, owner.user_id).await;

    assert_eq!(
        response.data,
        Value::Null,
        "an owner gets no violations back"
    );
    assert!(
        !response.errors.is_empty(),
        "the guard rejects a non-commissioner"
    );
}

/// Records one team's veteran-limit failure at a week 1 lock, as `lock_rosters` would.
async fn seed_recorded_violation(league: &TestLeague) -> deadline::Model {
    league
        .add_deadline(
            DeadlineKind::Week1RosterLock,
            central("2025-10-27T18:00:00"),
        )
        .await;
    let week_1_lock = deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        DeadlineKind::Week1RosterLock,
        &league.db,
    )
    .await
    .expect("find the week 1 lock");

    replace_violations_for_deadline(
        &week_1_lock,
        &[TeamRosterViolation {
            team_id: league.team_id,
            rule: RosterRule::VeteranOrRookieLimit,
            message: "23 of a possible 22 veteran or rookie contracts".to_owned(),
        }],
        &league.db,
    )
    .await
    .expect("record the violation");
    week_1_lock
}

/// Runs `rosterLockViolations` as `user_id` in the test league.
async fn run_query(league: &TestLeague, user_id: i64) -> Response {
    let schema: AppSchema = build_graphql_schema(league.db.clone());
    schema
        .execute(Request::new(QUERY).data(session_for(user_id, league.league_id).await))
        .await
}

fn field<'a>(data: &'a Value, name: &str) -> &'a Value {
    let Value::Object(object) = data else {
        panic!("expected an object, got {data:?}");
    };
    &object[name]
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
