//! The season-start wizard runs in the window BEFORE a roster lock fires, so which deadline it
//! legalizes against cannot be read off the clock: the last passed deadline is the previous one and
//! carries the previous period's rules (rules §11.2 regular-season limits vs the preseason limit).
//!
//! Naming the deadline is not choosing it, though: only the upcoming roster lock is a legal
//! argument, so a passed lock, a keeper deadline or a post-season kind cannot be named to run the
//! batch under another period's rules.
//!
//! The single-move mutations take the same argument and validate it the same way: one move and a
//! batched one have to agree on which week they belong to.

use std::sync::Arc;

use async_graphql::{Request, Value};
use chrono::{Days, Utc};
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    deadline::{self, DeadlineKind},
    deadline_queries, league,
    sea_orm::{ActiveValue, EntityTrait},
    team_update_queries,
    team_user::LeagueRole,
};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;
/// Rules §11.2: a roster carries at most 22 veteran or rookie-scale contracts.
const VET_OR_ROOKIE_LIMIT: usize = 22;

#[tokio::test]
async fn the_wizard_legalizes_against_the_named_deadline_not_the_last_passed_one() {
    let Some(league) = TestLeague::create("legalize_roster_deadline", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    // The keeper deadline has passed; the final roster lock has not, so reading the clock is wrong.
    league
        .add_deadline(
            DeadlineKind::PreseasonKeeper,
            central("2025-09-01T12:00:00"),
        )
        .await;
    let upcoming_lock = Utc::now()
        .checked_add_days(Days::new(30))
        .expect("30 days from now")
        .fixed_offset();
    league
        .add_deadline(DeadlineKind::PreseasonFinalRosterLock, upcoming_lock)
        .await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    // One contract over the 22-man limit, i.e. the roster the wizard exists to legalize.
    let contracts = add_roster_contracts(&league, VET_OR_ROOKIE_LIMIT + 1).await;
    let to_ir = contracts[0].id;
    let ir_move = format!("{{contractId: {to_ir}, kind: MOVE_TO_IR}}");

    let lock_id = deadline_id(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    // The named lock checks the regular-season branch, so 23 veteran contracts is illegal there.
    let no_moves = run(&schema, &legalize(league.team_id, lock_id, ""), &session).await;
    assert_eq!(no_moves, Err("ROSTER_ILLEGAL".to_owned()));

    // The refusal names the rule, so the wizard can point at the rule the roster broke.
    let violations =
        error_extension(&schema, &legalize(league.team_id, lock_id, ""), &session).await;
    let Some(Value::List(violations)) = violations else {
        panic!("expected a list of violations, got {violations:?}");
    };
    let [Value::Object(violation)] = violations.as_slice() else {
        panic!("expected exactly one violation, got {violations:?}");
    };
    assert_eq!(violation["rule"], Value::from("VETERAN_OR_ROOKIE_LIMIT"));
    assert_eq!(violation["teamId"], Value::from(league.team_id));
    assert!(
        violation["message"].to_string().contains("22"),
        "the message should name the limit: {violation:?}"
    );

    let with_ir_move = run(
        &schema,
        &legalize(league.team_id, lock_id, &ir_move),
        &session,
    )
    .await;
    assert!(
        with_ir_move.is_ok(),
        "expected the batch to apply: {with_ir_move:?}"
    );
    assert_eq!(
        ir_contract_count(&league).await,
        1,
        "the move should have put one contract on IR"
    );

    // A deadline belongs to exactly one league, so another league's is not a legal argument.
    let foreign_id = foreign_league_deadline(&league).await;
    let foreign = run(&schema, &legalize(league.team_id, foreign_id, ""), &session).await;
    assert_eq!(foreign, Err("NOT_FOUND".to_owned()));
}

/// Every other deadline row in the league carries another period's rules, so naming one would run
/// the batch under those: the IR guard passes unconditionally before the season (rules 10.3.1),
/// drops are penalty-free at the keeper deadline (8.3.3), and the post-season kinds resolve the
/// higher cap (4.2.3). Only the upcoming lock is a legal argument.
#[tokio::test]
async fn only_the_upcoming_roster_lock_is_a_legal_argument() {
    let Some(league) = TestLeague::create("legalize_roster_lock_choice", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonKeeper,
            central("2025-09-01T12:00:00"),
        )
        .await;
    // A lock of a settled week, and a deadline that is no lock at all.
    league
        .add_deadline(
            DeadlineKind::Week1RosterLock,
            central("2025-09-15T18:00:00"),
        )
        .await;
    league
        .add_deadline(DeadlineKind::SeasonEnd, central("2026-06-01T18:00:00"))
        .await;
    let upcoming_lock = Utc::now()
        .checked_add_days(Days::new(30))
        .expect("30 days from now")
        .fixed_offset();
    league
        .add_deadline(DeadlineKind::PreseasonFinalRosterLock, upcoming_lock)
        .await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    let contracts = add_roster_contracts(&league, 1).await;
    let to_ir = contracts[0].id;
    let ir_move = format!("{{contractId: {to_ir}, kind: MOVE_TO_IR}}");
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    for (case, kind, expected) in [
        (
            "the keeper deadline",
            DeadlineKind::PreseasonKeeper,
            "roster moves count towards a roster lock, and this deadline is not one",
        ),
        (
            "a post-season deadline",
            DeadlineKind::SeasonEnd,
            "roster moves count towards a roster lock, and this deadline is not one",
        ),
        (
            "a lock that already fired",
            DeadlineKind::Week1RosterLock,
            "roster moves count towards the upcoming roster lock, and this is not it",
        ),
    ] {
        let named = deadline_id(&league, kind).await;
        let mutation = legalize(league.team_id, named, &ir_move);
        assert_eq!(
            run(&schema, &mutation, &session).await,
            Err("BAD_REQUEST".to_owned()),
            "{case} should be rejected"
        );
        assert_eq!(message(&schema, &mutation, &session).await, expected);
    }

    assert_eq!(
        ir_contract_count(&league).await,
        0,
        "no rejected call should have applied its move"
    );

    // The upcoming lock is the one argument that works, so the wizard flow still runs.
    let lock_id = deadline_id(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let accepted = run(
        &schema,
        &legalize(league.team_id, lock_id, &ir_move),
        &session,
    )
    .await;
    assert!(
        accepted.is_ok(),
        "expected the batch to apply: {accepted:?}"
    );
}

/// A single move used to read the clock, so in the window before a lock fires it ran under the
/// previous week's rules and filed itself under the previous week. It names its lock now, and the
/// row it writes is stamped with that lock, which is what the week filter reads back.
#[tokio::test]
async fn a_single_move_counts_towards_the_named_upcoming_lock() {
    let Some(league) = TestLeague::create("single_move_deadline", END_OF_SEASON_YEAR).await else {
        return;
    };
    // The last passed deadline is a settled week's lock, so reading the clock names the wrong one.
    league
        .add_deadline(
            DeadlineKind::Week1RosterLock,
            central("2025-10-20T18:00:00"),
        )
        .await;
    let upcoming_lock = Utc::now()
        .checked_add_days(Days::new(7))
        .expect("7 days from now")
        .fixed_offset();
    league
        .add_deadline(DeadlineKind::PreseasonFinalRosterLock, upcoming_lock)
        .await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    let contracts = add_roster_contracts(&league, 1).await;
    let to_ir = contracts[0].id;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let settled_id = deadline_id(&league, DeadlineKind::Week1RosterLock).await;
    let settled = move_to_ir(to_ir, settled_id);
    assert_eq!(
        run(&schema, &settled, &session).await,
        Err("BAD_REQUEST".to_owned()),
        "a settled week's lock should be rejected"
    );
    assert_eq!(
        message(&schema, &settled, &session).await,
        "roster moves count towards the upcoming roster lock, and this is not it"
    );
    assert_eq!(
        ir_contract_count(&league).await,
        0,
        "the rejected call should not have applied its move"
    );

    let lock_id = deadline_id(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let accepted = run(&schema, &move_to_ir(to_ir, lock_id), &session).await;
    assert!(accepted.is_ok(), "expected the move to apply: {accepted:?}");
    assert_eq!(ir_contract_count(&league).await, 1);

    // The week filter reads the transaction's deadline, so the move has to be filed under the lock.
    let filed_under_lock = team_update_queries::find_team_updates_by_team(
        league.team_id,
        None,
        Some(lock_id),
        &league.db,
    )
    .await
    .expect("load the week's moves");
    assert_eq!(
        filed_under_lock.len(),
        1,
        "the move should be filed under the lock it named: {filed_under_lock:?}"
    );
}

fn move_to_ir(contract_id: i64, deadline_id: i64) -> String {
    format!(
        "mutation {{ moveContractToIr(contractId: {contract_id}, deadlineId: {deadline_id}) {{ id }} }}"
    )
}

fn legalize(team_id: i64, deadline_id: i64, moves: &str) -> String {
    format!(
        "mutation {{ legalizeRoster(teamId: {team_id}, deadlineId: {deadline_id}, moves: [{moves}]) {{ id }} }}"
    )
}

/// `count` $1 contracts owned by the league's team, i.e. roster filler that never breaks the cap.
///
/// Rookie extensions because the harness writes year 4, which a veteran contract does not allow;
/// both kinds count the same against the 22-man limit.
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

/// How many of the team's active contracts sit on IR. A move to IR writes a new contract row in
/// the chain, so the id the caller sent is not the row that ends up flagged.
async fn ir_contract_count(league: &TestLeague) -> usize {
    contract_queries::find_active_contracts_for_team(league.team_id, &league.db)
        .await
        .expect("load the team's contracts")
        .iter()
        .filter(|contract_model| contract_model.is_ir)
        .count()
}

/// A roster lock in a second league in the same database, i.e. an id the caller cannot use.
async fn foreign_league_deadline(league: &TestLeague) -> i64 {
    let foreign_league_id = league::Entity::insert(league::ActiveModel {
        name: ActiveValue::Set("Other league".to_owned()),
        ..Default::default()
    })
    .exec(&league.db)
    .await
    .expect("insert the other league")
    .last_insert_id;

    deadline::Entity::insert(deadline::ActiveModel {
        date_time: ActiveValue::Set(central("2025-10-20T18:00:00")),
        kind: ActiveValue::Set(DeadlineKind::PreseasonFinalRosterLock),
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

/// Runs one mutation as the session's user, returning its field value or the error's stable code.
async fn run(schema: &AppSchema, mutation: &str, session: &Session) -> Result<Value, String> {
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
    Ok(response.data)
}

/// The message of a failing mutation's error, i.e. what the owner is told.
async fn message(schema: &AppSchema, mutation: &str, session: &Session) -> String {
    let response = schema
        .execute(Request::new(mutation).data(session.clone()))
        .await;
    response
        .errors
        .first()
        .expect("the mutation should fail")
        .message
        .clone()
}

/// The `violations` extension of a failing mutation's error, i.e. the machine-readable payload.
async fn error_extension(schema: &AppSchema, mutation: &str, session: &Session) -> Option<Value> {
    let response = schema
        .execute(Request::new(mutation).data(session.clone()))
        .await;
    let error = response.errors.first().expect("the mutation should fail");
    error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("violations"))
        .cloned()
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
