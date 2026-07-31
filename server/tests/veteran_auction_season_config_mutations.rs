//! The veteran auction's two per-season inputs (§6.3.6) are entered through GraphQL, so the
//! commissioner guard, the implicit season, and replace-on-re-entry are all resolver behaviour.

use std::sync::Arc;

use async_graphql::{Request, Value};
use fbkl_entity::{
    auction_schedule_queries::{find_min_bid_tiers, find_veteran_auction_ranked_player_ids},
    deadline::DeadlineKind,
    team_user::LeagueRole,
};
use fbkl_server::{AppSchema, build_graphql_schema};
use fbkl_test_support::{TestLeague, central};
use tower_sessions::{MemoryStore, Session};

const END_OF_SEASON_YEAR: i16 = 2026;

#[tokio::test]
async fn the_commissioner_enters_a_season_of_tiers_and_rankings() {
    let Some(league) = TestLeague::create("vet_auction_config_mutations", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    // The current season is read off the most recent passed deadline, not a mutation argument.
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    let commissioner = league.add_team_user(LeagueRole::LeagueCommissioner).await;
    let owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let first_player_id = league.add_veteran_player("Best Vet").await;
    let second_player_id = league.add_veteran_player("Second Vet").await;

    let schema = build_graphql_schema(league.db.clone());
    let commissioner_session = session_for(commissioner.user_id, league.league_id).await;
    let owner_session = session_for(owner.user_id, league.league_id).await;

    let tiers_response = run(
        &schema,
        "mutation { setVeteranAuctionMinBidTiers(minBidAmounts: [20, 15, 10, 5]) }",
        &commissioner_session,
    )
    .await;
    assert_eq!(tiers_response, Ok(int_list(&[20, 15, 10, 5])));
    assert_eq!(entered_tiers(&league).await, vec![20, 15, 10, 5]);

    let ranking_response = run(
        &schema,
        &format!(
            "mutation {{ setVeteranAuctionRanking(playerIds: [{first_player_id}, {second_player_id}]) }}"
        ),
        &commissioner_session,
    )
    .await;
    assert_eq!(
        ranking_response,
        Ok(int_list(&[first_player_id, second_player_id]))
    );
    assert_eq!(
        entered_ranking(&league).await,
        vec![first_player_id, second_player_id]
    );

    // Re-entry replaces the season's list rather than appending to it.
    run(
        &schema,
        "mutation { setVeteranAuctionMinBidTiers(minBidAmounts: [30, 12]) }",
        &commissioner_session,
    )
    .await
    .expect("re-enter the tiers");
    assert_eq!(entered_tiers(&league).await, vec![30, 12]);
    run(
        &schema,
        &format!(
            "mutation {{ setVeteranAuctionRanking(playerIds: [{second_player_id}, {first_player_id}]) }}"
        ),
        &commissioner_session,
    )
    .await
    .expect("re-enter the ranking");
    assert_eq!(
        entered_ranking(&league).await,
        vec![second_player_id, first_player_id]
    );

    // Tiers must descend: the ladder slides down them, so an ascending list has no bottom.
    let ascending_tiers = run(
        &schema,
        "mutation { setVeteranAuctionMinBidTiers(minBidAmounts: [5, 10]) }",
        &commissioner_session,
    )
    .await;
    assert_eq!(ascending_tiers, Err("BAD_REQUEST".to_owned()));
    let empty_ranking = run(
        &schema,
        "mutation { setVeteranAuctionRanking(playerIds: []) }",
        &commissioner_session,
    )
    .await;
    assert_eq!(empty_ranking, Err("BAD_REQUEST".to_owned()));

    // A team owner is not a commissioner, and a refused call must not have written anything.
    let owner_tiers = run(
        &schema,
        "mutation { setVeteranAuctionMinBidTiers(minBidAmounts: [9, 8]) }",
        &owner_session,
    )
    .await;
    assert_eq!(owner_tiers, Err("FORBIDDEN".to_owned()));
    let owner_ranking = run(
        &schema,
        &format!("mutation {{ setVeteranAuctionRanking(playerIds: [{first_player_id}]) }}"),
        &owner_session,
    )
    .await;
    assert_eq!(owner_ranking, Err("FORBIDDEN".to_owned()));
    assert_eq!(entered_tiers(&league).await, vec![30, 12]);
    assert_eq!(
        entered_ranking(&league).await,
        vec![second_player_id, first_player_id]
    );
}

#[tokio::test]
async fn season_config_is_locked_once_the_pool_is_assembled() {
    let Some(league) = TestLeague::create("vet_auction_config_locked", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    league
        .add_deadline(
            DeadlineKind::PreseasonVeteranAuctionStart,
            central("2025-09-01T12:00:00"),
        )
        .await;
    let commissioner = league.add_team_user(LeagueRole::LeagueCommissioner).await;
    let player_id = league.add_veteran_player("Locked Vet").await;

    let schema = build_graphql_schema(league.db.clone());
    let session = session_for(commissioner.user_id, league.league_id).await;

    run(
        &schema,
        "mutation { setVeteranAuctionMinBidTiers(minBidAmounts: [20, 10]) }",
        &session,
    )
    .await
    .expect("enter the tiers before the auction starts");

    // A schedule row is what pool assembly writes, so its existence marks the auction as started.
    league
        .add_schedule_row(player_id, central("2025-09-02T00:00:00").date_naive(), 20)
        .await;

    let locked_tiers = run(
        &schema,
        "mutation { setVeteranAuctionMinBidTiers(minBidAmounts: [30, 12]) }",
        &session,
    )
    .await;
    assert_eq!(locked_tiers, Err("VETERAN_AUCTION_STARTED".to_owned()));
    let locked_ranking = run(
        &schema,
        &format!("mutation {{ setVeteranAuctionRanking(playerIds: [{player_id}]) }}"),
        &session,
    )
    .await;
    assert_eq!(locked_ranking, Err("VETERAN_AUCTION_STARTED".to_owned()));
    assert_eq!(entered_tiers(&league).await, vec![20, 10]);
    assert_eq!(entered_ranking(&league).await, Vec::<i64>::new());
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

    let Value::Object(fields) = response.data else {
        panic!("mutation returned no fields: {mutation}");
    };
    Ok(fields
        .values()
        .next()
        .cloned()
        .unwrap_or_else(|| panic!("mutation returned no fields: {mutation}")))
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

fn int_list(values: &[i64]) -> Value {
    Value::List(values.iter().map(|value| Value::from(*value)).collect())
}

async fn entered_tiers(league: &TestLeague) -> Vec<i16> {
    find_min_bid_tiers(league.league_id, END_OF_SEASON_YEAR, &league.db)
        .await
        .expect("read the entered tiers")
        .into_iter()
        .map(|tier| tier.min_bid_amount)
        .collect()
}

async fn entered_ranking(league: &TestLeague) -> Vec<i64> {
    find_veteran_auction_ranked_player_ids(league.league_id, END_OF_SEASON_YEAR, &league.db)
        .await
        .expect("read the entered ranking")
}
