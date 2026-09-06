//! A transaction is submitted in the window BEFORE a roster lock fires, so the lock it counts
//! towards has to be named rather than read off the clock: the last passed deadline is the previous
//! one and would file the moves in the wrong week.
//!
//! The named lock is the week, not the ruleset. A transaction is judged by the limits in force when
//! it is made: 32 contracts through the preseason (rules §5.1.2), 22 veteran or rookie-scale ones
//! in season (§11.2). The final roster lock is the only lock in the whole preseason, so reading its
//! own limits would refuse every preseason roster on its way to legalization.
//!
//! Naming the deadline is not choosing it, though: only the upcoming roster lock is a legal
//! argument, so a passed lock, a keeper deadline or a post-season kind cannot be named to run the
//! transaction under another period's rules.
//!
//! The single-move mutations take the same argument and validate it the same way: one move and a
//! batched one have to agree on which week they belong to.

use std::sync::Arc;

use async_graphql::{Request, Value};
use chrono::{Days, Utc};
use fbkl_constants::league_rules::PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT;
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    deadline::{self, DeadlineKind},
    deadline_queries, league,
    sea_orm::{ActiveValue, EntityTrait},
    team_update_queries,
    team_user::LeagueRole,
};
use fbkl_logic::deadline_processing::roster_lock::{RosterRule, validate_team_roster};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;
/// Rules §11.2: a roster carries at most 22 veteran or rookie-scale contracts.
const VET_OR_ROOKIE_LIMIT: usize = 22;

#[tokio::test]
async fn a_preseason_transaction_is_judged_against_the_preseason_roster_limit() {
    let Some(league) = TestLeague::create("submit_transaction_deadline", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    // The final roster lock is the only lock in the whole preseason, so the rules a transaction is
    // judged by come from the window it is made in: the rookie draft has started and the lock has
    // not fired, which rules 5.1.2 allow 32 contracts through.
    league
        .add_deadline(DeadlineKind::PreseasonStart, central("2025-08-01T12:00:00"))
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonKeeper,
            central("2025-09-01T12:00:00"),
        )
        .await;
    league
        .add_deadline(
            DeadlineKind::PreseasonRookieDraftStart,
            central("2025-09-20T12:00:00"),
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

    // One contract over the preseason's own limit, i.e. the roster a transaction has to legalize.
    let preseason_limit = usize::try_from(PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT)
        .expect("the preseason limit is a small positive number");
    let contracts = add_roster_contracts(&league, preseason_limit + 1).await;

    let lock_id = deadline_id(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let no_moves = run(&schema, &submit(league.team_id, lock_id, ""), &session).await;
    assert_eq!(no_moves, Err("ROSTER_ILLEGAL".to_owned()));

    // The refusal names the preseason rule, not the 22-man limit the coming lock will impose.
    let violations = error_extension(&schema, &submit(league.team_id, lock_id, ""), &session).await;
    let Some(Value::List(violations)) = violations else {
        panic!("expected a list of violations, got {violations:?}");
    };
    let [Value::Object(violation)] = violations.as_slice() else {
        panic!("expected one violation, got {violations:?}");
    };
    assert_eq!(violation["rule"], Value::from("PRESEASON_ROSTER_LIMIT"));
    assert_eq!(violation["teamId"], Value::from(league.team_id));
    assert!(
        violation["message"].to_string().contains("32"),
        "the message should name the limit: {violation:?}"
    );

    // One drop is enough, even though 32 contracts is far over the coming lock's 22.
    let lone_drop = run(&schema, &drop_contract(contracts[0].id, lock_id), &session).await;
    assert!(
        lone_drop.is_ok(),
        "a lone drop should legalize a preseason roster: {lone_drop:?}"
    );
    assert_eq!(active_contract_count(&league).await, preseason_limit);

    // The lock still holds the roster to 22 when it fires, which is what the preseason legalizes to.
    let final_lock = deadline_queries::find_deadline_by_id(lock_id, &league.db)
        .await
        .expect("find the lock");
    let at_the_lock = validate_team_roster(league.team_id, &final_lock, &league.db)
        .await
        .expect("validate the roster at the lock");
    assert_eq!(
        at_the_lock
            .iter()
            .map(|violation| violation.rule)
            .collect::<Vec<_>>(),
        vec![RosterRule::VeteranOrRookieLimit],
        "the lock itself judges the roster by the regular-season limits"
    );

    // A deadline belongs to one league, so another league's is not a legal argument.
    let foreign_id = foreign_league_deadline(&league).await;
    let foreign = run(&schema, &submit(league.team_id, foreign_id, ""), &session).await;
    assert_eq!(foreign, Err("NOT_FOUND".to_owned()));
}

/// Every other deadline row in the league carries another period's rules, so naming one would run
/// the batch under those: the IR guard passes unconditionally before the season (rules 10.3.1),
/// drops are penalty-free at the keeper deadline (8.3.3), and the post-season kinds resolve the
/// higher cap (4.2.3). Only the upcoming lock is a legal argument.
#[tokio::test]
async fn only_the_upcoming_roster_lock_is_a_legal_argument() {
    let Some(league) =
        TestLeague::create("submit_transaction_lock_choice", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonKeeper,
            central("2025-09-01T12:00:00"),
        )
        .await;
    // The window the preseason's transactions are judged in, i.e. the rules in force before the lock.
    league
        .add_deadline(
            DeadlineKind::PreseasonRookieDraftStart,
            central("2025-09-10T12:00:00"),
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
        let mutation = submit(league.team_id, named, &ir_move);
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

    // The upcoming lock is the one argument that works, so the submission still runs.
    let lock_id = deadline_id(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let accepted = run(
        &schema,
        &submit(league.team_id, lock_id, &ir_move),
        &session,
    )
    .await;
    assert!(
        accepted.is_ok(),
        "expected the batch to apply: {accepted:?}"
    );
}

/// The wizard was the first caller, but a transaction is the shape every week takes: rules §13.1.4
/// counts one in-season week's moves the same way, so the mutation is not tied to the preseason
/// lock. Its moves share one transaction number, and the week's next submission takes the next.
#[tokio::test]
async fn a_transaction_applies_at_an_in_season_lock_and_numbers_its_moves_as_one() {
    let Some(league) = TestLeague::create("submit_transaction_in_season", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    add_season_under_way(&league).await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    let contracts = add_roster_contracts(&league, 4).await;
    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let two_drops = format!(
        "{}, {}",
        drop_move(contracts[0].id),
        drop_move(contracts[1].id)
    );
    let applied = run(
        &schema,
        &submit(league.team_id, lock_id, &two_drops),
        &session,
    )
    .await;
    assert!(
        applied.is_ok(),
        "an in-season lock should take a transaction: {applied:?}"
    );
    assert_eq!(
        stored_transaction_numbers(&league, lock_id).await,
        vec![Some(0), Some(0)],
        "both moves belong to the one transaction that applied them"
    );

    let next = run(
        &schema,
        &submit(league.team_id, lock_id, &drop_move(contracts[2].id)),
        &session,
    )
    .await;
    assert!(next.is_ok(), "expected a second transaction: {next:?}");
    assert_eq!(
        stored_transaction_numbers(&league, lock_id).await,
        vec![Some(0), Some(0), Some(1)],
        "the week's next submission is its next transaction"
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
    // The window the coming preseason lock's transactions are judged in.
    league
        .add_deadline(
            DeadlineKind::PreseasonRookieDraftStart,
            central("2025-09-10T12:00:00"),
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

    // The week filter reads the league event's deadline, so the move has to be filed under the lock.
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

/// A lone move is a transaction of one move (rules §13.1.4), so it is judged the way a batch is:
/// T1 refuses it when the roster it leaves is illegal, and it takes a transaction number of its own
/// rather than sharing the batch's before it.
#[tokio::test]
async fn a_single_move_is_judged_and_numbered_as_its_own_transaction() {
    let Some(league) = TestLeague::create("single_move_transaction", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    add_season_under_way(&league).await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    // Two contracts over the 22-man limit, so one drop is not enough to make the roster legal.
    let contracts = add_roster_contracts(&league, VET_OR_ROOKIE_LIMIT + 2).await;
    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let too_few = drop_contract(contracts[0].id, lock_id);
    assert_eq!(
        run(&schema, &too_few, &session).await,
        Err("ROSTER_ILLEGAL".to_owned()),
        "a lone drop that leaves the roster illegal is refused"
    );
    assert_eq!(
        active_contract_count(&league).await,
        VET_OR_ROOKIE_LIMIT + 2,
        "the refused drop should not have persisted"
    );
    assert!(
        stored_transaction_numbers(&league, lock_id)
            .await
            .is_empty(),
        "a refused move writes no row to number"
    );

    let two_drops = format!(
        "{}, {}",
        drop_move(contracts[0].id),
        drop_move(contracts[1].id)
    );
    let batch = run(
        &schema,
        &submit(league.team_id, lock_id, &two_drops),
        &session,
    )
    .await;
    assert!(batch.is_ok(), "expected the batch to legalize: {batch:?}");

    let lone = run(&schema, &drop_contract(contracts[2].id, lock_id), &session).await;
    assert!(
        lone.is_ok(),
        "a lone drop off a legal roster should apply: {lone:?}"
    );
    assert_eq!(
        stored_transaction_numbers(&league, lock_id).await,
        vec![Some(0), Some(0), Some(1)],
        "the lone move is its own transaction, not part of the batch before it"
    );
}

/// An RDI player moved back stateside stays an RD contract, so the move counts against neither the
/// 22-man limit nor T2 (rules §11.7.1). The batch carries it alongside a drop to prove it files as
/// an ordinary move of the same transaction.
#[tokio::test]
async fn a_batch_moves_an_rdi_contract_back_to_rd() {
    let Some(league) = TestLeague::create("submit_transaction_rdi", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    add_season_under_way(&league).await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    let contracts = add_roster_contracts(&league, VET_OR_ROOKIE_LIMIT).await;
    let overseas_player = league.add_veteran_player("Overseas Rookie").await;
    let rdi_contract = league
        .add_owned_contract(
            overseas_player,
            ContractKind::RookieDevelopmentInternational,
            1,
            league.team_id,
        )
        .await;
    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let moves = format!(
        "{}, {{contractId: {}, kind: MOVE_FROM_RDI}}",
        drop_move(contracts[0].id),
        rdi_contract.id
    );
    let batch = run(&schema, &submit(league.team_id, lock_id, &moves), &session).await;
    assert!(batch.is_ok(), "expected the RDI move to apply: {batch:?}");

    let kinds: Vec<ContractKind> =
        contract_queries::find_active_contracts_for_team(league.team_id, &league.db)
            .await
            .expect("load the team's contracts")
            .iter()
            .map(|contract_model| contract_model.kind)
            .filter(|kind| !matches!(kind, ContractKind::RookieExtension))
            .collect();
    assert_eq!(
        kinds,
        vec![ContractKind::RookieDevelopment],
        "the RDI contract should now be an RD one"
    );
    assert_eq!(
        stored_transaction_numbers(&league, lock_id).await,
        vec![Some(0), Some(0)],
        "both moves belong to the one submitted transaction"
    );
}

/// A batch may move one player more than once, and every move writes a replacement contract row.
/// So the id the client sent names a row an earlier move in the same batch has already replaced,
/// and the client cannot name the live one: it does not exist when the batch is submitted. The
/// mutation resolves each id to its chain's latest row before it applies the move.
///
/// The T2 refusal is what proves the resolution: an unresolved id would name a `Replaced` row and
/// fail as `ContractNotActive` instead. Rules §13.1.5.5 make an activation an acquisition, so
/// activating a player in order to drop him in the same transaction breaks T2.
#[tokio::test]
async fn a_batch_follows_the_contract_chain_between_moves_on_one_player() {
    let Some(league) = TestLeague::create("submit_transaction_chain", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    add_season_under_way(&league).await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    let contracts = add_roster_contracts(&league, 4).await;
    let named_id = contracts[0].id;
    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    let ir_move = format!("{{contractId: {named_id}, kind: MOVE_TO_IR}}");
    let parked = run(
        &schema,
        &submit(league.team_id, lock_id, &ir_move),
        &session,
    )
    .await;
    assert!(
        parked.is_ok(),
        "expected the move to IR to apply: {parked:?}"
    );
    assert_eq!(ir_contract_count(&league).await, 1);

    // The named id is two rows stale by the drop, so only a resolved chain root matches it to the
    // activation T2 refuses it for.
    let activate_then_drop = format!(
        "{{contractId: {named_id}, kind: ACTIVATE_FROM_IR}}, {}",
        drop_move(named_id)
    );
    let refusal = message(
        &schema,
        &submit(league.team_id, lock_id, &activate_then_drop),
        &session,
    )
    .await;
    assert!(
        refusal.contains("acquired in this transaction"),
        "activating a player in order to drop him breaks T2: {refusal}"
    );
    assert_eq!(
        ir_contract_count(&league).await,
        1,
        "the refused transaction leaves the player on IR"
    );
    assert_eq!(
        active_contract_count(&league).await,
        4,
        "the refused transaction drops nobody"
    );
    assert_eq!(
        stored_transaction_numbers(&league, lock_id).await,
        vec![Some(0)],
        "only the move to IR is numbered; the refused batch writes nothing"
    );

    let rookie = league.add_veteran_player("Rookie Dev").await;
    let rd_contract = league
        .add_owned_contract(rookie, ContractKind::RookieDevelopment, 1, league.team_id)
        .await;
    let activate_then_drop_rookie = format!(
        "{{contractId: {}, kind: ACTIVATE_ROOKIE}}, {}",
        rd_contract.id,
        drop_move(rd_contract.id)
    );
    let rookie_refusal = message(
        &schema,
        &submit(league.team_id, lock_id, &activate_then_drop_rookie),
        &session,
    )
    .await;
    assert!(
        rookie_refusal.contains("acquired in this transaction"),
        "activating a rookie in order to drop him breaks T2: {rookie_refusal}"
    );
}

fn move_to_ir(contract_id: i64, deadline_id: i64) -> String {
    format!(
        "mutation {{ moveContractToIr(contractId: {contract_id}, deadlineId: {deadline_id}) {{ id }} }}"
    )
}

fn drop_contract(contract_id: i64, deadline_id: i64) -> String {
    format!(
        "mutation {{ dropContract(contractId: {contract_id}, deadlineId: {deadline_id}) {{ id }} }}"
    )
}

/// The deadlines of a season already under way, with an in-season lock still to fire.
///
/// The keeper deadline and week 1's lock are settled, and the in-season lock prices its cap against
/// the free-agent auction end (rules §4.2.3), which a season missing that row cannot do.
async fn add_season_under_way(league: &TestLeague) {
    league
        .add_deadline(
            DeadlineKind::PreseasonKeeper,
            central("2025-09-01T12:00:00"),
        )
        .await;
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
    let upcoming_lock = Utc::now()
        .checked_add_days(Days::new(3))
        .expect("3 days from now")
        .fixed_offset();
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, upcoming_lock)
        .await;
}

fn submit(team_id: i64, deadline_id: i64, moves: &str) -> String {
    format!(
        "mutation {{ submitTransaction(teamId: {team_id}, deadlineId: {deadline_id}, moves: [{moves}]) {{ id }} }}"
    )
}

fn drop_move(contract_id: i64) -> String {
    format!("{{contractId: {contract_id}, kind: DROP}}")
}

/// Every transaction number stored for the team's week, oldest move first.
async fn stored_transaction_numbers(league: &TestLeague, deadline_id: i64) -> Vec<Option<i16>> {
    let mut week_moves = team_update_queries::find_team_updates_by_team(
        league.team_id,
        None,
        Some(deadline_id),
        &league.db,
    )
    .await
    .expect("load the week\'s moves");
    week_moves.sort_by_key(|team_update| team_update.id);
    week_moves
        .iter()
        .map(|team_update| team_update.transaction_number)
        .collect()
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

/// How many active contracts the team owns, i.e. what the 22-man limit counts.
async fn active_contract_count(league: &TestLeague) -> usize {
    contract_queries::find_active_contracts_for_team(league.team_id, &league.db)
        .await
        .expect("load the team's contracts")
        .len()
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
