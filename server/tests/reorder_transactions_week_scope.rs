//! A transaction order is positions 0..n, so it only makes sense within one week. The mutation
//! therefore names the week's lock deadline and takes that week's moves and no others: a list
//! mixing weeks would write positions that clash with the ones already stored for the other week.
//!
//! Order is not presentational any more - the transaction a move sits in decides what T1 and T2
//! judge it with - but rules §13.1.1 let an owner reorder freely, so the mutation stores whatever
//! grouping it is given and leaves an illegal week for the lock to record.

use std::sync::Arc;

use async_graphql::{Request, Value};
use fbkl_entity::{
    deadline::{self, DeadlineKind},
    deadline_queries,
    league_event::{self, LeagueEventKind},
    league_event_queries,
    sea_orm::{ActiveValue, EntityTrait},
    team_update::{self, TeamUpdateData, TeamUpdateStatus},
    team_update_queries, team_user,
};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;

#[tokio::test]
async fn a_transaction_order_covers_one_week_and_nothing_else() {
    let Some(league) =
        TestLeague::create("reorder_transactions_week_scope", END_OF_SEASON_YEAR).await
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
            DeadlineKind::InSeasonRosterLock,
            central("2025-10-27T18:00:00"),
        )
        .await;
    let owner = league.add_team_user(team_user::LeagueRole::TeamOwner).await;

    let last_week = deadline_id(&league, DeadlineKind::Week1RosterLock).await;
    let this_week = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let last_week_move = record_move(&league, last_week).await;
    let first = record_move(&league, this_week).await;
    let second = record_move(&league, this_week).await;

    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;
    let reorder = |deadline_id: i64, transactions: Vec<Vec<i64>>| {
        let transactions = format!("{transactions:?}");
        format!(
            "mutation {{ reorderTransactions(teamId: {}, deadlineId: {deadline_id}, orderedTransactions: {transactions}) {{ transactionNumber moves {{ id }} }} }}",
            league.team_id
        )
    };

    // Nothing has an order yet, so the rows carry no transaction number at all.
    assert_eq!(
        stored_transaction_numbers(&league, this_week).await,
        vec![(first, None), (second, None)]
    );

    let reversed = run(
        &schema,
        &reorder(this_week, vec![vec![second], vec![first]]),
        &session,
    )
    .await;
    assert_eq!(
        reversed.expect("the week's own moves are a legal order"),
        vec![(Some(0_i16), vec![second]), (Some(1), vec![first])],
        "one move per transaction stores the list positions and echoes them in that order"
    );

    let together = run(
        &schema,
        &reorder(this_week, vec![vec![first, second]]),
        &session,
    )
    .await;
    assert_eq!(
        together.expect("both moves in one transaction is a legal order"),
        vec![(Some(0), vec![first, second])],
        "moves put in one transaction share its number and come back as one group"
    );

    for (case, transactions) in [
        (
            "a move from another week",
            vec![vec![second], vec![first, last_week_move]],
        ),
        ("only some of the week's moves", vec![vec![first]]),
        (
            "the same move twice",
            vec![vec![first], vec![first, second]],
        ),
        ("an empty transaction", vec![vec![], vec![first, second]]),
    ] {
        let rejected = run(&schema, &reorder(this_week, transactions), &session).await;
        assert_eq!(
            rejected.err().as_deref(),
            Some("BAD_REQUEST"),
            "{case} should be rejected"
        );
    }

    // The other week orders on its own, so position 0 exists once per week, not once per team.
    run(
        &schema,
        &reorder(last_week, vec![vec![last_week_move]]),
        &session,
    )
    .await
    .expect("last week's own move is a legal order");
    assert_eq!(
        stored_transaction_numbers(&league, last_week).await,
        vec![(last_week_move, Some(0))]
    );
    assert_eq!(
        stored_transaction_numbers(&league, this_week).await,
        vec![(first, Some(0)), (second, Some(0))],
        "reordering another week leaves this week's transactions alone"
    );

    let foreign = run(
        &schema,
        &reorder(foreign_league_deadline(&league).await, vec![]),
        &session,
    )
    .await;
    assert_eq!(foreign.err().as_deref(), Some("NOT_FOUND"));
}

/// The stored id and transaction number of every move in one week, oldest row first.
async fn stored_transaction_numbers(
    league: &TestLeague,
    deadline_id: i64,
) -> Vec<(i64, Option<i16>)> {
    let mut moves: Vec<(i64, Option<i16>)> = team_update_queries::find_team_updates_by_team(
        league.team_id,
        None,
        Some(deadline_id),
        &league.db,
    )
    .await
    .expect("load the week's moves")
    .iter()
    .map(|model| (model.id, model.transaction_number))
    .collect();
    moves.sort_unstable();
    moves
}

/// One pending move recorded against `deadline_id`, i.e. a row in that week's tray.
async fn record_move(league: &TestLeague, deadline_id: i64) -> i64 {
    let league_event_model = league_event_queries::insert_league_event(
        league_event::ActiveModel {
            end_of_season_year: ActiveValue::Set(END_OF_SEASON_YEAR),
            kind: ActiveValue::Set(LeagueEventKind::TeamUpdateDropContract),
            league_id: ActiveValue::Set(league.league_id),
            deadline_id: ActiveValue::Set(deadline_id),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert league_event");

    let data = TeamUpdateData::from_assets(vec![], vec![], 0, 0, 0, 0)
        .to_json()
        .expect("team update data as json");
    team_update_queries::insert_team_update(
        team_update::ActiveModel {
            data: ActiveValue::Set(data),
            effective_date: ActiveValue::Set(central("2025-10-21T18:00:00").date_naive()),
            status: ActiveValue::Set(TeamUpdateStatus::Pending),
            team_id: ActiveValue::Set(league.team_id),
            league_event_id: ActiveValue::Set(Some(league_event_model.id)),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert team update")
    .id
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

/// A lock deadline in a second league in the same database, i.e. an id the caller cannot use.
async fn foreign_league_deadline(league: &TestLeague) -> i64 {
    let foreign_league_id = fbkl_entity::league::Entity::insert(fbkl_entity::league::ActiveModel {
        name: ActiveValue::Set("Other league".to_owned()),
        ..Default::default()
    })
    .exec(&league.db)
    .await
    .expect("insert the other league")
    .last_insert_id;

    deadline::Entity::insert(deadline::ActiveModel {
        date_time: ActiveValue::Set(central("2025-10-20T18:00:00")),
        kind: ActiveValue::Set(DeadlineKind::InSeasonRosterLock),
        name: ActiveValue::Set("Other league lock".to_owned()),
        end_of_season_year: ActiveValue::Set(END_OF_SEASON_YEAR),
        league_id: ActiveValue::Set(foreign_league_id),
        ..Default::default()
    })
    .exec(&league.db)
    .await
    .expect("insert the other league's deadline")
    .last_insert_id
}

/// Runs `reorderTransactions`, returning each transaction it echoes back: its number and its move
/// ids.
async fn run(
    schema: &AppSchema,
    mutation: &str,
    session: &Session,
) -> Result<Vec<(Option<i16>, Vec<i64>)>, String> {
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
    Ok(transactions_of(&response.data, "reorderTransactions"))
}

fn transactions_of(data: &Value, field: &str) -> Vec<(Option<i16>, Vec<i64>)> {
    let Value::Object(data) = data else {
        panic!("expected an object, got {data:?}");
    };
    let Value::List(transactions) = &data[field] else {
        panic!("expected a list of transactions, got {:?}", data[field]);
    };
    transactions
        .iter()
        .map(|transaction| {
            let Value::Object(transaction) = transaction else {
                panic!("expected a transaction object, got {transaction:?}");
            };
            let transaction_number = number_of(&transaction["transactionNumber"])
                .map(|number| i16::try_from(number).expect("transaction_number fits in i16"));
            let Value::List(moves) = &transaction["moves"] else {
                panic!("expected a list of moves, got {:?}", transaction["moves"]);
            };
            let move_ids = moves
                .iter()
                .map(|team_update| {
                    let Value::Object(team_update) = team_update else {
                        panic!("expected a move object, got {team_update:?}");
                    };
                    number_of(&team_update["id"]).expect("id as a number")
                })
                .collect();
            (transaction_number, move_ids)
        })
        .collect()
}

fn number_of(value: &Value) -> Option<i64> {
    value.clone().into_json().expect("value as json").as_i64()
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
