//! A trade add belongs to the week of the lock still to fire, not the next deadline of any kind.
//!
//! `process_trade` used to stamp its transaction with the next deadline of ANY kind, so a non-lock
//! deadline sitting between the trade and the Monday lock (a `FreeAgentAuctionEnd`, say) filed the
//! `AddViaTrade` update outside the lock's week. Both same-week guards read the week off that
//! deadline, so the acquiring owner could park the pickup straight on IR (rules 10.3.1/10.3.2,
//! 11.7) and the 8.3.7 drop guard could not see the add at all.
//!
//! A season with no lock left has no week for the add either, so that case is pinned here too: it
//! has to fail with a typed error the resolver can name, not an opaque one it reports as a 500.

use chrono::{Days, Utc};
use fbkl_entity::{
    contract::{self, ContractKind, ContractStatus},
    contract_queries,
    deadline::{self, DeadlineKind},
    deadline_queries,
    sea_orm::prelude::DateTimeWithTimeZone,
    team_update_queries,
    team_user::{self, LeagueRole},
    trade, trade_asset,
};
use fbkl_logic::{
    ir::move_contract_to_ir,
    trade::{MissingUpcomingRosterLock, accept_trade, propose_trade},
};
use fbkl_test_support::TestLeague;

const END_OF_SEASON_YEAR: i16 = 2026;

#[tokio::test]
async fn a_trade_add_is_filed_under_the_upcoming_lock_and_cannot_go_straight_to_ir() {
    let Some(league) = TestLeague::create("trade_add_lock_week", END_OF_SEASON_YEAR).await else {
        return;
    };
    let now = Utc::now().fixed_offset();
    // A non-lock deadline before the lock: the old code stamped the trade with this one.
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_from_now(1))
        .await;
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, days_from_now(3))
        .await;
    let (proposed_trade, receiving_owner, receiving_team_id, traded_contract) =
        propose_one_contract_trade(&league).await;
    accept_trade(proposed_trade, &receiving_owner, &now, &league.db)
        .await
        .expect("accept trade")
        .expect("both teams have responded, so the trade processes");

    let lock = deadline_of_kind(&league, DeadlineKind::InSeasonRosterLock).await;
    let auction_end = deadline_of_kind(&league, DeadlineKind::FreeAgentAuctionEnd).await;
    // The week query both same-week guards run: the receiving team's moves under a deadline.
    assert_eq!(
        week_move_count(&league, receiving_team_id, auction_end.id).await,
        0,
        "the trade add must not be filed under a non-lock deadline"
    );
    assert_eq!(
        week_move_count(&league, receiving_team_id, lock.id).await,
        1,
        "the trade add belongs to the week of the lock it will be judged at"
    );

    let received_contract = active_contract_in_chain(&league, traded_contract.id).await;
    assert_eq!(received_contract.team_id, Some(receiving_team_id));
    let ir_error = move_contract_to_ir(received_contract, &lock, &league.db)
        .await
        .expect_err("a player received via trade cannot go straight to IR in the add's own week");
    assert!(
        ir_error.to_string().contains("straight to IR"),
        "the refusal should name the rule: {ir_error}"
    );
}

#[tokio::test]
async fn accepting_a_trade_with_no_lock_left_names_the_missing_lock_deadlines() {
    let Some(league) = TestLeague::create("trade_no_upcoming_lock", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    let now = Utc::now().fixed_offset();
    // The season's only deadline is not a lock, so no lock is left for the add to be judged at.
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, days_from_now(1))
        .await;
    let (proposed_trade, receiving_owner, _, _) = propose_one_contract_trade(&league).await;

    let error = accept_trade(proposed_trade, &receiving_owner, &now, &league.db)
        .await
        .expect_err("a trade cannot be processed with no lock left to file its adds under");

    assert_eq!(
        error.downcast_ref::<MissingUpcomingRosterLock>(),
        Some(&MissingUpcomingRosterLock {
            league_id: league.league_id,
            end_of_season_year: END_OF_SEASON_YEAR,
        }),
        "the failure has to be typed so the resolver reports it instead of a 500: {error}"
    );
}

/// One contract moving from the test league's own team to a second team, proposed and awaiting the
/// receiving owner's response. Returns the trade, that owner, their team, and the traded contract.
async fn propose_one_contract_trade(
    league: &TestLeague,
) -> (trade::Model, team_user::Model, i64, contract::Model) {
    let sending_owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let receiving_team_id = league.add_team("Receiving team").await;
    let receiving_owner = league
        .add_team_user_for_team(receiving_team_id, LeagueRole::TeamOwner)
        .await;
    let player_id = league.add_veteran_player("Traded Player").await;
    let traded_contract = league
        .add_owned_contract(player_id, ContractKind::RookieExtension, 10, league.team_id)
        .await;

    let proposed_trade = propose_trade(
        league.league_id,
        END_OF_SEASON_YEAR,
        &sending_owner,
        &[receiving_team_id],
        vec![trade_asset::Model::from_contract(
            None,
            traded_contract.id,
            trade_asset::FromTeamId(league.team_id),
            trade_asset::ToTeamId(receiving_team_id),
        )],
        &league.db,
    )
    .await
    .expect("propose trade");

    (
        proposed_trade,
        receiving_owner,
        receiving_team_id,
        traded_contract,
    )
}

fn days_from_now(days: u64) -> DateTimeWithTimeZone {
    Utc::now()
        .checked_add_days(Days::new(days))
        .expect("days from now")
        .fixed_offset()
}

async fn deadline_of_kind(league: &TestLeague, kind: DeadlineKind) -> deadline::Model {
    deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        kind,
        &league.db,
    )
    .await
    .expect("find deadline")
}

async fn week_move_count(league: &TestLeague, team_id: i64, deadline_id: i64) -> usize {
    team_update_queries::find_team_updates_by_team(team_id, None, Some(deadline_id), &league.db)
        .await
        .expect("find team updates")
        .len()
}

async fn active_contract_in_chain(league: &TestLeague, contract_id: i64) -> contract::Model {
    contract_queries::find_contract_chain(contract_id, &league.db)
        .await
        .expect("find contract chain")
        .into_iter()
        .find(|chain_contract| chain_contract.status == ContractStatus::Active)
        .expect("an active contract in the chain")
}
