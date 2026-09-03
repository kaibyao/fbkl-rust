//! A trade and one owner's accommodating drops are one transaction (rules §12.5.3, §13.1.4).
//!
//! Drops used to arrive after the trade as separate mutations, which cannot work under the
//! transaction model: the trade would leave an illegal roster behind, and each follow-up drop would
//! then fail T1 on its own. So the proposer submits their drops with the proposal and every other
//! owner submits theirs with their accept, and the accept judges each involved team's legs plus
//! that team's drops as one transaction.

use chrono::{Days, Utc};
use fbkl_entity::{
    contract::{self, ContractKind, ContractStatus},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::prelude::DateTimeWithTimeZone,
    team_update_queries,
    team_user::{self, LeagueRole},
    trade::{self, TradeStatus},
    trade_asset,
    trade_queries::find_trade_by_id,
};
use fbkl_logic::{
    roster::RosterMoveRejection,
    trade::{TradeLegality, accept_trade, propose_trade},
};
use fbkl_test_support::{TestLeague, central};

const END_OF_SEASON_YEAR: i16 = 2026;
/// Rules §11.2: a roster carries at most 22 veteran or rookie-scale contracts.
const VET_OR_ROOKIE_LIMIT: usize = 22;

#[tokio::test]
async fn a_trades_legs_and_the_accepters_drop_are_one_transaction() {
    let Some(league) = TestLeague::create("trade_drops_one_transaction", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    add_season_under_way(&league).await;
    let sending_owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let receiving_team_id = league.add_team("Receiving team").await;
    let receiving_owner = league
        .add_team_user_for_team(receiving_team_id, LeagueRole::TeamOwner)
        .await;

    let traded_contract = add_contracts(&league, league.team_id, 1, "Sent").await[0].clone();
    // The accepter is already full, so the incoming contract needs a drop to make room.
    let receiving_roster =
        add_contracts(&league, receiving_team_id, VET_OR_ROOKIE_LIMIT, "Kept").await;

    let proposed_trade =
        propose(&league, &sending_owner, receiving_team_id, &traded_contract).await;
    accept_trade(
        proposed_trade,
        &receiving_owner,
        &now(),
        &[receiving_roster[0].id],
        TradeLegality::JudgeNow,
        &league.db,
    )
    .await
    .expect("the accepter's drop makes room for the incoming contract")
    .expect("both teams have responded, so the trade processes");

    let lock_id = deadline_id(&league, DeadlineKind::InSeasonRosterLock).await;
    assert_eq!(
        transaction_numbers(&league, receiving_team_id, lock_id).await,
        vec![Some(0), Some(0)],
        "the accepter's incoming leg and their drop are one transaction"
    );
    assert_eq!(
        transaction_numbers(&league, league.team_id, lock_id).await,
        vec![Some(0)],
        "the sender's leg is their own transaction of the week"
    );
    assert_eq!(
        active_contract_count(&league, receiving_team_id).await,
        VET_OR_ROOKIE_LIMIT,
        "the drop leaves the accepter's roster at the limit"
    );
}

#[tokio::test]
async fn an_accept_whose_drops_do_not_cover_the_incoming_legs_is_refused() {
    let Some(league) = TestLeague::create("trade_drops_missing", END_OF_SEASON_YEAR).await else {
        return;
    };
    add_season_under_way(&league).await;
    let sending_owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let receiving_team_id = league.add_team("Receiving team").await;
    let receiving_owner = league
        .add_team_user_for_team(receiving_team_id, LeagueRole::TeamOwner)
        .await;

    let traded_contract = add_contracts(&league, league.team_id, 1, "Sent").await[0].clone();
    add_contracts(&league, receiving_team_id, VET_OR_ROOKIE_LIMIT, "Kept").await;

    let proposed_trade =
        propose(&league, &sending_owner, receiving_team_id, &traded_contract).await;
    let trade_id = proposed_trade.id;
    let error = accept_trade(
        proposed_trade,
        &receiving_owner,
        &now(),
        &[],
        TradeLegality::JudgeNow,
        &league.db,
    )
    .await
    .expect_err("a 23rd contract with no drop leaves the accepter's roster illegal");

    match error.downcast_ref::<RosterMoveRejection>() {
        Some(RosterMoveRejection::TransactionLeavesRosterIllegal {
            team_id,
            violations,
        }) => {
            assert_eq!(*team_id, receiving_team_id, "the refusal names the team");
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.message.contains("22")),
                "the refusal names the rule: {violations:?}"
            );
        }
        other => panic!("expected a T1 rejection, got {other:?} from {error}"),
    }

    // Nothing the accept applied persists: the accept returned before its commit.
    assert_eq!(
        find_trade_by_id(trade_id, &league.db)
            .await
            .expect("the trade is still on record")
            .status,
        TradeStatus::Proposed
    );
    assert_eq!(
        active_contract_count(&league, receiving_team_id).await,
        VET_OR_ROOKIE_LIMIT
    );
    assert_eq!(
        contract_queries::find_contract_by_id(traded_contract.id, &league.db)
            .await
            .expect("the traded contract row is untouched")
            .team_id,
        Some(league.team_id),
        "the trade's legs rolled back with the refusal"
    );
}

#[tokio::test]
async fn dropping_a_contract_the_trade_brings_in_is_refused_by_t2() {
    let Some(league) = TestLeague::create("trade_drops_own_add", END_OF_SEASON_YEAR).await else {
        return;
    };
    add_season_under_way(&league).await;
    let sending_owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let receiving_team_id = league.add_team("Receiving team").await;
    let receiving_owner = league
        .add_team_user_for_team(receiving_team_id, LeagueRole::TeamOwner)
        .await;

    let traded_contract = add_contracts(&league, league.team_id, 1, "Sent").await[0].clone();
    // One under the limit, so taking the contract in and dropping it both leave a legal roster:
    // only T2 can refuse this, which is the point.
    add_contracts(&league, receiving_team_id, VET_OR_ROOKIE_LIMIT - 1, "Kept").await;

    let proposed_trade =
        propose(&league, &sending_owner, receiving_team_id, &traded_contract).await;
    let error = accept_trade(
        proposed_trade,
        &receiving_owner,
        &now(),
        &[traded_contract.id],
        TradeLegality::JudgeNow,
        &league.db,
    )
    .await
    .expect_err("a contract acquired in this transaction cannot be dropped by it");

    assert!(
        matches!(
            error.downcast_ref::<RosterMoveRejection>(),
            Some(RosterMoveRejection::SameTransactionAddThenRemove { .. })
        ),
        "expected a T2 rejection, got {error}"
    );
}

#[tokio::test]
async fn a_multi_owner_trade_judges_every_involved_team() {
    let Some(league) = TestLeague::create("trade_drops_multi_owner", END_OF_SEASON_YEAR).await
    else {
        return;
    };
    add_season_under_way(&league).await;
    let sending_owner = league.add_team_user(LeagueRole::TeamOwner).await;
    let roomy_team_id = league.add_team("Roomy team").await;
    let roomy_owner = league
        .add_team_user_for_team(roomy_team_id, LeagueRole::TeamOwner)
        .await;
    let full_team_id = league.add_team("Full team").await;
    let full_owner = league
        .add_team_user_for_team(full_team_id, LeagueRole::TeamOwner)
        .await;

    let sent = add_contracts(&league, league.team_id, 2, "Sent").await;
    add_contracts(&league, full_team_id, VET_OR_ROOKIE_LIMIT, "Kept").await;

    let proposed_trade = propose_trade(
        league.league_id,
        END_OF_SEASON_YEAR,
        &sending_owner,
        &[roomy_team_id, full_team_id],
        vec![
            trade_asset::Model::from_contract(
                None,
                sent[0].id,
                trade_asset::FromTeamId(league.team_id),
                trade_asset::ToTeamId(roomy_team_id),
            ),
            trade_asset::Model::from_contract(
                None,
                sent[1].id,
                trade_asset::FromTeamId(league.team_id),
                trade_asset::ToTeamId(full_team_id),
            ),
        ],
        &[],
        &league.db,
    )
    .await
    .expect("propose a three-team trade");

    let trade_id = proposed_trade.id;
    let processed = accept_trade(
        proposed_trade,
        &roomy_owner,
        &now(),
        &[],
        TradeLegality::JudgeNow,
        &league.db,
    )
    .await
    .expect("the team with room accepts");
    assert!(
        processed.is_none(),
        "one team has yet to respond, so nothing processes"
    );

    let awaiting_trade = find_trade_by_id(trade_id, &league.db)
        .await
        .expect("the trade is still on record");
    let error = accept_trade(
        awaiting_trade,
        &full_owner,
        &now(),
        &[],
        TradeLegality::JudgeNow,
        &league.db,
    )
    .await
    .expect_err("the full team's transaction leaves its roster illegal");

    match error.downcast_ref::<RosterMoveRejection>() {
        Some(RosterMoveRejection::TransactionLeavesRosterIllegal { team_id, .. }) => {
            assert_eq!(
                *team_id, full_team_id,
                "every involved team is judged, and the refusal names the one that failed"
            );
        }
        other => panic!("expected a T1 rejection, got {other:?} from {error}"),
    }
    assert_eq!(
        find_trade_by_id(trade_id, &league.db)
            .await
            .expect("the trade is still on record")
            .status,
        TradeStatus::Proposed,
        "one team's refused transaction refuses the whole trade"
    );
}

/// One contract offered from the league's own team to `receiving_team_id`, awaiting that owner.
async fn propose(
    league: &TestLeague,
    sending_owner: &team_user::Model,
    receiving_team_id: i64,
    traded_contract: &contract::Model,
) -> trade::Model {
    propose_trade(
        league.league_id,
        END_OF_SEASON_YEAR,
        sending_owner,
        &[receiving_team_id],
        vec![trade_asset::Model::from_contract(
            None,
            traded_contract.id,
            trade_asset::FromTeamId(league.team_id),
            trade_asset::ToTeamId(receiving_team_id),
        )],
        &[],
        &league.db,
    )
    .await
    .expect("propose trade")
}

async fn add_contracts(
    league: &TestLeague,
    team_id: i64,
    count: usize,
    name_prefix: &str,
) -> Vec<contract::Model> {
    let mut contracts = Vec::with_capacity(count);
    for index in 0..count {
        let player_id = league
            .add_veteran_player(&format!("{name_prefix} {team_id}-{index}"))
            .await;
        contracts.push(
            league
                .add_owned_contract(player_id, ContractKind::RookieExtension, 1, team_id)
                .await,
        );
    }
    contracts
}

/// The season's in-season lock is three days out, with the deadlines the in-season cap is read
/// from already passed (rules §4.2.3).
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
    league
        .add_deadline(
            DeadlineKind::InSeasonRosterLock,
            Utc::now()
                .checked_add_days(Days::new(3))
                .expect("3 days from now")
                .fixed_offset(),
        )
        .await;
}

/// A team's stored transaction numbers for one week, oldest move first.
async fn transaction_numbers(
    league: &TestLeague,
    team_id: i64,
    deadline_id: i64,
) -> Vec<Option<i16>> {
    let mut week_moves = team_update_queries::find_team_updates_by_team(
        team_id,
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

async fn active_contract_count(league: &TestLeague, team_id: i64) -> usize {
    contract_queries::find_active_contracts_for_team(team_id, &league.db)
        .await
        .expect("load the team's active contracts")
        .iter()
        .filter(|contract_model| contract_model.status == ContractStatus::Active)
        .count()
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

fn now() -> DateTimeWithTimeZone {
    Utc::now().fixed_offset()
}
