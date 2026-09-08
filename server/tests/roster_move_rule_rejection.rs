//! A roster move a league rule refuses is the owner's fault, not a server fault.
//!
//! The logic layer's guards carry a specific message ("is not in IR"), and the resolver used to
//! throw all of it away behind a bare INTERNAL, so an owner could not tell a rule rejection from a
//! database outage. A rejection now carries the `ROSTER_MOVE_REJECTED` code plus the rule message,
//! and only genuine faults stay INTERNAL.

use std::sync::Arc;

use async_graphql::Request;
use chrono::{Days, Utc};
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries,
    team_user::LeagueRole,
};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::TestLeague;
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;

/// Rules §10.3.2: only a contract on IR can come off it, and the refusal names the rule.
#[tokio::test]
async fn a_rule_rejection_names_the_rule_instead_of_reading_as_a_server_fault() {
    let Some(league) = TestLeague::create("roster_move_rejection", END_OF_SEASON_YEAR).await else {
        return;
    };
    // The lock this move gets judged at.
    let upcoming_lock = Utc::now()
        .checked_add_days(Days::new(3))
        .expect("3 days from now")
        .fixed_offset();
    league
        .add_deadline(DeadlineKind::Week1RosterLock, upcoming_lock)
        .await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let fresh_contract = add_roster_contract(&league).await;

    let lock_id = deadline_id(&league, DeadlineKind::Week1RosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    // This contract was never on IR, so there is nothing to activate off it.
    let (code, message) = run(
        &schema,
        &activate_from_ir(fresh_contract.id, lock_id),
        &session,
    )
    .await;
    assert_eq!(code, Some("ROSTER_MOVE_REJECTED".to_owned()));
    assert!(
        message.contains("not in IR"),
        "the owner should be told which rule stopped the move: {message}"
    );
    assert_eq!(
        ir_contract_count(&league).await,
        0,
        "the rejected move should not have been applied"
    );
}

fn activate_from_ir(contract_id: i64, deadline_id: i64) -> String {
    format!(
        "mutation {{ activateContractFromIr(contractId: {contract_id}, deadlineId: {deadline_id}) {{ id }} }}"
    )
}

/// One $1 contract owned by the league's team, i.e. roster filler that never breaks the cap.
async fn add_roster_contract(league: &TestLeague) -> contract::Model {
    let player_id = league.add_veteran_player("Fresh Add").await;
    league
        .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
        .await
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

/// How many of the team's active contracts sit on IR.
async fn ir_contract_count(league: &TestLeague) -> usize {
    contract_queries::find_active_contracts_for_team(league.team_id, &league.db)
        .await
        .expect("load the team's contracts")
        .iter()
        .filter(|contract_model| contract_model.is_ir)
        .count()
}

/// Runs one failing mutation, returning its stable error code and the message the owner sees.
async fn run(schema: &AppSchema, mutation: &str, session: &Session) -> (Option<String>, String) {
    let response = schema
        .execute(Request::new(mutation).data(session.clone()))
        .await;
    let error = response
        .errors
        .first()
        .expect("the mutation should be rejected");
    let code = error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("code"))
        .map(|code| code.to_string().trim_matches('"').to_owned());

    (code, error.message.clone())
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
