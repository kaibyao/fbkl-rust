//! `teamWeek` and `reorderWeeklyMoves` have to agree on which moves make up a week.
//!
//! Drops, trades and auction wins are recorded as Done at once, while an IR move waits for the lock
//! as Pending. Rules §13.1.1 order covers the whole week, so `teamWeek` lists every status and
//! `reorderWeeklyMoves` takes that same list. When `teamWeek` filtered to Pending, the ids it gave a
//! client were a subset of the week, and the mutation rejected them.

use std::sync::Arc;

use async_graphql::{Request, Value};
use fbkl_entity::{
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::ActiveValue,
    team_update::{self, TeamUpdateData, TeamUpdateStatus},
    team_update_queries, team_user,
    transaction::{self, TransactionKind},
    transaction_queries,
};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;

#[tokio::test]
async fn a_week_of_done_and_pending_moves_lists_and_reorders_as_one_set() {
    let Some(league) = TestLeague::create("team_week_moves_round_trip", END_OF_SEASON_YEAR).await
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
    let owner = league.add_team_user(team_user::LeagueRole::TeamOwner).await;
    let lock_id = deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        DeadlineKind::InSeasonRosterLock,
        &league.db,
    )
    .await
    .expect("find the in-season lock")
    .id;

    let drop = record_move(
        &league,
        lock_id,
        TransactionKind::TeamUpdateDropContract,
        TeamUpdateStatus::Done,
    )
    .await;
    let auction_win = record_move(
        &league,
        lock_id,
        TransactionKind::AuctionDone,
        TeamUpdateStatus::Done,
    )
    .await;
    let pending_ir = record_move(
        &league,
        lock_id,
        TransactionKind::TeamUpdateToIr,
        TeamUpdateStatus::Pending,
    )
    .await;

    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let listed = team_week_move_ids(&schema, &league, lock_id, &session).await;
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    let mut expected = vec![drop, auction_win, pending_ir];
    expected.sort_unstable();
    assert_eq!(
        sorted, expected,
        "the week is the drop, the auction win and the IR move, whatever their status"
    );

    let reversed: Vec<i64> = listed.iter().rev().copied().collect();
    let reordered = reorder(&schema, &league, lock_id, &reversed, &session).await;
    assert_eq!(
        reordered.expect("the ids teamWeek gave back are a legal order"),
        reversed,
        "the mutation takes exactly the set teamWeek exposes"
    );
    assert_eq!(
        team_week_move_ids(&schema, &league, lock_id, &session).await,
        reversed,
        "teamWeek reads the saved order back"
    );

    let pending_only = reorder(&schema, &league, lock_id, &[pending_ir], &session).await;
    assert_eq!(
        pending_only.err().as_deref(),
        Some("BAD_REQUEST"),
        "a subset of the week is still not an order, which is why teamWeek cannot filter"
    );
}

/// The ids of one week's moves as `teamWeek` lists them, in the order it lists them.
async fn team_week_move_ids(
    schema: &AppSchema,
    league: &TestLeague,
    deadline_id: i64,
    session: &Session,
) -> Vec<i64> {
    let query = format!(
        "query {{ teamWeek(teamId: {}, deadlineId: {deadline_id}) {{ moves {{ id status }} }} }}",
        league.team_id
    );
    let response = schema
        .execute(Request::new(query).data(session.clone()))
        .await;
    assert!(
        response.errors.is_empty(),
        "teamWeek has to answer: {:?}",
        response.errors
    );
    let Value::Object(data) = &response.data else {
        panic!("expected an object, got {:?}", response.data);
    };
    let Value::Object(team_week) = &data["teamWeek"] else {
        panic!("expected a teamWeek object");
    };
    move_ids(&team_week["moves"])
}

/// Runs `reorderWeeklyMoves`, returning the ids it echoes back or the error code it rejected with.
async fn reorder(
    schema: &AppSchema,
    league: &TestLeague,
    deadline_id: i64,
    ordered_ids: &[i64],
    session: &Session,
) -> Result<Vec<i64>, String> {
    let mutation = format!(
        "mutation {{ reorderWeeklyMoves(teamId: {}, deadlineId: {deadline_id}, orderedTeamUpdateIds: {ordered_ids:?}) {{ id status }} }}",
        league.team_id
    );
    let response = schema
        .execute(Request::new(mutation).data(session.clone()))
        .await;
    if let Some(error) = response.errors.first() {
        let code = error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .map_or_else(|| error.message.clone(), ToString::to_string);
        return Err(code.trim_matches('"').to_owned());
    }
    let Value::Object(data) = &response.data else {
        panic!("expected an object, got {:?}", response.data);
    };
    Ok(move_ids(&data["reorderWeeklyMoves"]))
}

fn move_ids(moves: &Value) -> Vec<i64> {
    let Value::List(moves) = moves else {
        panic!("expected a list of moves, got {moves:?}");
    };
    moves
        .iter()
        .map(|team_update| {
            let Value::Object(team_update) = team_update else {
                panic!("expected a move object, got {team_update:?}");
            };
            team_update["id"]
                .clone()
                .into_json()
                .expect("id as json")
                .as_i64()
                .expect("id as a number")
        })
        .collect()
}

/// One move of the given kind and status recorded against `deadline_id`.
async fn record_move(
    league: &TestLeague,
    deadline_id: i64,
    kind: TransactionKind,
    status: TeamUpdateStatus,
) -> i64 {
    let transaction_model = transaction_queries::insert_transaction(
        transaction::ActiveModel {
            end_of_season_year: ActiveValue::Set(END_OF_SEASON_YEAR),
            kind: ActiveValue::Set(kind),
            league_id: ActiveValue::Set(league.league_id),
            deadline_id: ActiveValue::Set(deadline_id),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert transaction");

    let data = TeamUpdateData::from_assets(vec![], vec![], 0, 0, 0, 0)
        .to_json()
        .expect("team update data as json");
    team_update_queries::insert_team_update(
        team_update::ActiveModel {
            data: ActiveValue::Set(data),
            effective_date: ActiveValue::Set(central("2025-10-21T18:00:00").date_naive()),
            status: ActiveValue::Set(status),
            team_id: ActiveValue::Set(league.team_id),
            transaction_id: ActiveValue::Set(Some(transaction_model.id)),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert team update")
    .id
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
