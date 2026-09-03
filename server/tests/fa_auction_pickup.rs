//! An owner turns a week's free-agent auction wins into contracts by picking them up, together
//! with the drops that make room (rules §8.3.5-§8.3.7).
//!
//! The close records a win and signs nothing, so the pickup is where the in-season cap and roster
//! check actually happens: an owner may bid above their free cap as long as they free the space if
//! they win. All of a week's wins go on together, which is what makes the Mitchell/Alvarado case a
//! T2 refusal - dropping one of the two wins to fit the other puts an add and its removal in one
//! transaction.

use std::sync::Arc;

use async_graphql::{Request, Value};
use chrono::{Days, Utc};
use fbkl_entity::{
    auction::{AuctionKind, AuctionStatus},
    auction_queries,
    contract::{self, ContractKind},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries, team_update_queries,
    team_user::{self, LeagueRole},
};
use fbkl_logic::auction::start_new_auction_for_nba_player;
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;
/// Rules §11.2: a roster carries at most 22 veteran or rookie-scale contracts.
const VET_OR_ROOKIE_LIMIT: usize = 22;
const WINNING_BID: i16 = 5;

/// The Mitchell/Alvarado case: an owner one slot short of their two wins cannot drop one of the two
/// to fit the other, because §8.3.5 says every win must be picked up and T2 refuses a transaction
/// that removes what it just acquired. Dropping someone already on the roster is what works, and
/// the wins and that drop are then one transaction.
#[tokio::test]
async fn a_pickup_signs_every_win_with_its_drops_as_one_transaction() {
    let Some(league) = TestLeague::create("fa_auction_pickup", END_OF_SEASON_YEAR).await else {
        return;
    };
    add_season_under_way(&league).await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;

    // One slot free, two wins waiting: the second needs a drop to fit.
    let roster = add_roster_contracts(&league, VET_OR_ROOKIE_LIMIT - 1).await;
    let won = [
        add_won_auction(&league, &owner, "Donovan Mitchell").await,
        add_won_auction(&league, &owner, "Jose Alvarado").await,
    ];

    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(owner.user_id, league.league_id).await;

    assert_eq!(
        run(&schema, &pick_up(lock_id, &[]), &session).await,
        Err("ROSTER_ILLEGAL".to_owned()),
        "signing both wins with no drop leaves the roster one over the limit"
    );

    let dropping_its_own_win = pick_up(lock_id, &[won[1]]);
    assert_eq!(
        run(&schema, &dropping_its_own_win, &session).await,
        Err("ROSTER_MOVE_REJECTED".to_owned()),
        "dropping one of the week's own wins to fit the other is refused by T2"
    );
    let refusal = message(&schema, &dropping_its_own_win, &session).await;
    assert!(
        refusal.contains("acquired in this transaction"),
        "the refusal should be T2, not another rule: {refusal}"
    );
    assert_eq!(
        active_contract_count(&league).await,
        VET_OR_ROOKIE_LIMIT - 1,
        "no refused pickup should have signed anything"
    );
    assert!(
        stored_transaction_numbers(&league, lock_id)
            .await
            .is_empty(),
        "a refused pickup writes no move to number"
    );

    let picked_up = run(&schema, &pick_up(lock_id, &[roster[0].id]), &session).await;
    assert!(
        picked_up.is_ok(),
        "a drop off the standing roster makes room: {picked_up:?}"
    );
    assert_eq!(
        active_contract_count(&league).await,
        VET_OR_ROOKIE_LIMIT,
        "both wins are signed and the drop leaves the roster at the limit"
    );
    assert!(
        won_auction_ids(&league, &owner).await.is_empty(),
        "a picked-up win is no longer waiting for a pickup"
    );
    assert_eq!(
        stored_transaction_numbers(&league, lock_id).await,
        vec![Some(0), Some(0), Some(0)],
        "both signings and the drop are one transaction"
    );
}

/// Signs `pick_up_auction_wins`, dropping the contracts named. A drop of one of the week's own wins
/// names the auctioned contract, since the signed row does not exist when the owner submits.
fn pick_up(deadline_id: i64, drop_contract_ids: &[i64]) -> String {
    let drops = drop_contract_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "mutation {{ pickUpAuctionWins(deadlineId: {deadline_id}, dropContractIds: [{drops}]) {{ id }} }}"
    )
}

/// An auction the owner's team has won but not picked up, i.e. what an in-season close leaves
/// behind. Returns the auctioned contract's id, which is what a drop of that win names.
async fn add_won_auction(league: &TestLeague, owner: &team_user::Model, name: &str) -> i64 {
    let player_id = league.add_veteran_player(name).await;
    let pooled_contract = league
        .add_unowned_contract(
            player_id,
            ContractKind::UnrestrictedFreeAgentVeteran,
            WINNING_BID,
        )
        .await;
    let auction = start_new_auction_for_nba_player(
        &pooled_contract,
        league.league_id,
        END_OF_SEASON_YEAR,
        central("2025-10-27T18:00:00"),
        AuctionKind::InSeasonFreeAgent,
        WINNING_BID,
        &league.db,
    )
    .await
    .expect("start the in-season FA auction");
    auction_queries::insert_auction_bid(auction.id, owner.id, WINNING_BID, None, &league.db)
        .await
        .expect("insert the winning bid");
    auction_queries::update_auction_status(auction.id, AuctionStatus::Won, &league.db)
        .await
        .expect("record the win");

    pooled_contract.id
}

async fn won_auction_ids(league: &TestLeague, owner: &team_user::Model) -> Vec<i64> {
    auction_queries::find_won_auctions_for_team(
        owner.team_id,
        league.league_id,
        END_OF_SEASON_YEAR,
        &league.db,
    )
    .await
    .expect("read the team's unsigned wins")
    .iter()
    .map(|(auction_model, _)| auction_model.id)
    .collect()
}

/// The deadlines of a season already under way, with an in-season lock still to fire. The lock
/// prices its cap against the free-agent auction end (rules §4.2.3).
async fn add_season_under_way(league: &TestLeague) {
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

/// Every transaction number stored for the team's week, oldest move first.
async fn stored_transaction_numbers(league: &TestLeague, deadline_id: i64) -> Vec<Option<i16>> {
    let mut week_moves = team_update_queries::find_team_updates_by_team(
        league.team_id,
        None,
        Some(deadline_id),
        &league.db,
    )
    .await
    .expect("load the week's moves");
    week_moves.sort_by_key(|team_update| team_update.id);
    week_moves
        .iter()
        .map(|team_update| team_update.transaction_number)
        .collect()
}

async fn active_contract_count(league: &TestLeague) -> usize {
    contract_queries::find_active_contracts_for_team(league.team_id, &league.db)
        .await
        .expect("load the team's contracts")
        .len()
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
