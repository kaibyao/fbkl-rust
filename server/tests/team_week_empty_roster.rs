//! An empty roster is a legal roster: a brand-new team owns nothing and `teamWeek` has to answer.
//!
//! The `INTERNAL` this file was written for turned out not to come from the empty roster at all. It
//! comes from an incomplete season of deadlines: the cap for an `InSeasonRosterLock` is the $210
//! regular-season limit before `FreeAgentAuctionEnd` and the $230 post-season one after (rules
//! §4.2.3/§8.1), so `deadline::Model::get_salary_cap` reads that row and fails when the season has
//! none. Both cases are asserted below, the second so the cause stays named.

use std::sync::Arc;

use async_graphql::{Request, Response, Value};
use fbkl_entity::{deadline::DeadlineKind, deadline_queries, team_user};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;

#[tokio::test]
async fn a_team_that_owns_nothing_has_a_legal_week() {
    let Some(league) = TestLeague::create("team_week_empty_roster", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::InSeasonRosterLock,
            central("2025-10-27T18:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::FreeAgentAuctionEnd,
            central("2026-03-01T18:00:00"),
        )
        .await;

    let response = run_team_week(&league).await;

    assert!(
        response.errors.is_empty(),
        "an empty roster is a legal state, not a fault: {:?}",
        response.errors
    );
    let team_week = field(&response.data, "teamWeek");
    assert_eq!(field(team_week, "isLegal"), &Value::Boolean(true));
    assert_eq!(field(team_week, "contracts"), &Value::List(vec![]));
    assert_eq!(field(team_week, "pendingMoves"), &Value::List(vec![]));
    let Value::List(rule_flags) = field(team_week, "ruleLegality") else {
        panic!("expected a flag per rule");
    };
    assert!(
        !rule_flags.is_empty()
            && rule_flags
                .iter()
                .all(|flag| field(flag, "isLegal") == &Value::Boolean(true)),
        "no rule is broken by an empty roster: {rule_flags:?}"
    );
}

#[tokio::test]
async fn a_season_with_no_auction_end_deadline_cannot_price_the_in_season_cap() {
    let Some(league) =
        TestLeague::create("team_week_no_auction_end_deadline", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::InSeasonRosterLock,
            central("2025-10-27T18:00:00"),
        )
        .await;

    let response = run_team_week(&league).await;

    let code = response.errors.first().and_then(|error| {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .map(ToString::to_string)
    });
    assert_eq!(
        code.as_deref()
            .map(|code| code.trim_matches('"').to_owned()),
        Some("INTERNAL".to_owned()),
        "a season missing FreeAgentAuctionEnd is bad league data, and the cap cannot be resolved \
         without it — seed the row in fixtures rather than making the cap lookup optional"
    );
}

/// Runs `teamWeek` for the test league's own team at its in-season lock, as its owner.
async fn run_team_week(league: &TestLeague) -> Response {
    let owner = league.add_team_user(team_user::LeagueRole::TeamOwner).await;
    let deadline_id = deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        DeadlineKind::InSeasonRosterLock,
        &league.db,
    )
    .await
    .expect("find the in-season lock")
    .id;

    let schema: AppSchema = build_graphql_schema(league.db.clone());
    let query = format!(
        "query {{ teamWeek(teamId: {}, deadlineId: {deadline_id}) {{ isLegal contracts {{ id }} pendingMoves {{ id }} ruleLegality {{ rule isLegal }} }} }}",
        league.team_id
    );
    schema
        .execute(Request::new(query).data(session_for(owner.user_id, league.league_id).await))
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
