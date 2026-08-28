//! Cover for the weekly-moves rules that only hold across several moves at once: a week's moves are
//! judged as a set at roster lock, not one at a time as they are made (rules §8.3.7, §10.1.2,
//! §13.1.2).
//!
//! Each test seeds its own league with a preseason final roster lock and a week 1 roster lock, so
//! "in-season" and "preseason" can both be exercised without a full season's deadlines.

use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries,
    deadline::{self, DeadlineKind},
    deadline_queries, roster_lock_violation_queries,
    sea_orm::{ActiveValue, EntityTrait},
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries,
    transaction::{self, TransactionKind},
    transaction_queries,
};
use fbkl_logic::{
    deadline_processing::{RosterRule, TeamRosterViolation, lock_rosters, validate_league_rosters},
    drop_contract::drop_contract_from_team,
    ir::move_contract_to_ir,
};
use fbkl_test_support::{TestLeague, central};

const END_OF_SEASON_YEAR: i16 = 2026;
/// Rules §11.2: a roster carries at most 22 veteran or rookie-scale contracts.
const VET_OR_ROOKIE_LIMIT: usize = 22;
const PRESEASON_LOCK: &str = "2025-10-20T18:00:00";
const WEEK_1_LOCK: &str = "2025-10-27T18:00:00";
const WEEK_2_LOCK: &str = "2025-11-03T18:00:00";
const FA_AUCTION_END: &str = "2026-03-01T18:00:00";

/// A league with the two roster locks these tests move contracts against.
async fn weekly_moves_league(test_name: &str) -> Option<TestLeague> {
    let league = TestLeague::create(test_name, END_OF_SEASON_YEAR).await?;
    league
        .add_deadline(
            DeadlineKind::PreseasonFinalRosterLock,
            central(PRESEASON_LOCK),
        )
        .await;
    league
        .add_deadline(DeadlineKind::Week1RosterLock, central(WEEK_1_LOCK))
        .await;
    Some(league)
}

async fn deadline_of(league: &TestLeague, kind: DeadlineKind) -> deadline::Model {
    deadline_queries::find_deadline_for_season_by_type(
        league.league_id,
        END_OF_SEASON_YEAR,
        kind,
        &league.db,
    )
    .await
    .expect("find deadline")
}

/// `count` $1 contracts owned by `team_id`, i.e. roster filler that never breaks the cap.
///
/// Rookie extensions because the harness writes year 4, which a veteran contract does not allow;
/// both kinds count the same against the 22-man limit.
async fn add_roster_contracts(
    league: &TestLeague,
    team_id: i64,
    count: usize,
    name_prefix: &str,
) -> Vec<contract::Model> {
    let mut contracts = Vec::with_capacity(count);
    for index in 0..count {
        let player_id = league
            .add_veteran_player(&format!("{name_prefix} {index}"))
            .await;
        contracts.push(
            league
                .add_owned_contract(player_id, ContractKind::RookieExtension, 1, team_id)
                .await,
        );
    }
    contracts
}

/// One `team_update` to seed, i.e. a roster move recorded without running the code that makes it.
struct RecordedMove<'a> {
    deadline_model: &'a deadline::Model,
    kind: TransactionKind,
    status: TeamUpdateStatus,
    /// The roster the update commits, i.e. `all_contract_ids`.
    roster_contract_ids: Vec<i64>,
    /// The moves the update records, as `(contract_id, update_type)` pairs.
    contract_moves: Vec<(i64, ContractUpdateType)>,
}

/// Inserts `recorded` as a `team_update` on `team_id`, skipping the logic that normally writes it.
async fn record_move(
    league: &TestLeague,
    team_id: i64,
    recorded: RecordedMove<'_>,
) -> team_update::Model {
    let RecordedMove {
        deadline_model,
        kind,
        status,
        roster_contract_ids,
        contract_moves,
    } = recorded;

    let transaction_model = transaction_queries::insert_transaction(
        transaction::ActiveModel {
            end_of_season_year: ActiveValue::Set(END_OF_SEASON_YEAR),
            kind: ActiveValue::Set(kind),
            league_id: ActiveValue::Set(league.league_id),
            deadline_id: ActiveValue::Set(deadline_model.id),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert transaction");

    let contract_updates = contract_moves
        .into_iter()
        .map(|(contract_id, update_type)| ContractUpdate {
            contract_id,
            update_type,
            player_name_at_time: String::new(),
            player_team_abbr_at_time: String::new(),
            player_team_name_at_time: String::new(),
        })
        .collect();
    let team_update_data = TeamUpdateData::from_assets(
        roster_contract_ids,
        vec![TeamUpdateAsset::Contracts(contract_updates)],
        0,
        0,
        0,
        0,
    );

    team_update_queries::insert_team_update(
        team_update::ActiveModel {
            data: ActiveValue::Set(
                team_update_data
                    .to_json()
                    .expect("team update data as json"),
            ),
            effective_date: ActiveValue::Set(deadline_model.date_time.date_naive()),
            status: ActiveValue::Set(status),
            team_id: ActiveValue::Set(team_id),
            transaction_id: ActiveValue::Set(Some(transaction_model.id)),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert team update")
}

/// Records an auction win for `contract_model` against `deadline_model`, i.e. a pending same-week
/// add, without running an auction.
async fn record_auction_add(
    league: &TestLeague,
    contract_model: &contract::Model,
    deadline_model: &deadline::Model,
) -> team_update::Model {
    let team_id = contract_model.team_id.expect("added contract has a team");
    record_move(
        league,
        team_id,
        RecordedMove {
            deadline_model,
            kind: TransactionKind::AuctionDone,
            status: TeamUpdateStatus::Pending,
            roster_contract_ids: vec![contract_model.id],
            contract_moves: vec![(contract_model.id, ContractUpdateType::AddViaAuction)],
        },
    )
    .await
}

fn broken_rules(violations: &[TeamRosterViolation]) -> Vec<RosterRule> {
    violations.iter().map(|violation| violation.rule).collect()
}

#[tokio::test]
async fn a_week_illegal_only_in_the_middle_still_locks() {
    let Some(league) = weekly_moves_league("weekly_moves_transiently_illegal").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let roster =
        add_roster_contracts(&league, league.team_id, VET_OR_ROOKIE_LIMIT, "Holdover").await;

    // The auction win lands before the drop that makes room for it, i.e. a 23-man roster mid-week.
    let auction_win_player_id = league.add_veteran_player("Auction win").await;
    let auction_win = league
        .add_owned_contract(
            auction_win_player_id,
            ContractKind::RookieExtension,
            1,
            league.team_id,
        )
        .await;
    record_auction_add(&league, &auction_win, &week_1_lock).await;
    assert_eq!(
        broken_rules(
            &validate_league_rosters(&week_1_lock, &league.db)
                .await
                .expect("validate the mid-week roster")
        ),
        vec![RosterRule::VeteranOrRookieLimit]
    );

    drop_contract_from_team(roster[0].clone(), &week_1_lock, &league.db)
        .await
        .expect("drop a holdover to make room");

    let violations = lock_rosters(&week_1_lock, &league.db)
        .await
        .expect("lock rosters");

    assert!(
        violations.is_empty(),
        "the week is legal by lock time: {violations:?}"
    );
    let locked_team_updates =
        team_update_queries::find_team_updates_by_team(league.team_id, None, None, &league.db)
            .await
            .expect("read the team's updates");
    assert!(
        locked_team_updates
            .iter()
            .all(|team_update_model| team_update_model.status == TeamUpdateStatus::Done),
        "every move of a legal week locks"
    );
}

#[tokio::test]
async fn direct_to_ir_is_preseason_only() {
    let Some(league) = weekly_moves_league("weekly_moves_direct_to_ir").await else {
        return;
    };
    let preseason_lock = deadline_of(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let player_id = league.add_veteran_player("Fresh signing").await;
    let signing = league
        .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
        .await;

    let rejection = move_contract_to_ir(signing.clone(), &week_1_lock, &league.db)
        .await
        .expect_err("in-season direct-to-IR is rejected");
    assert!(
        rejection.to_string().contains("straight to IR"),
        "unexpected rejection: {rejection}"
    );

    let ir_contract = move_contract_to_ir(signing, &preseason_lock, &league.db)
        .await
        .expect("the preseason final roster lock allows direct-to-IR");
    assert!(ir_contract.is_ir);
}

/// Rules §10.3.1: a contract acquired in-season must sit on the 22-man roster before it may go to
/// IR, and the add's own Done `team_update` is not proof that it ever did.
#[tokio::test]
async fn an_in_season_add_cannot_go_straight_to_ir() {
    let Some(league) = weekly_moves_league("weekly_moves_in_season_add_to_ir").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;

    for (add_kind, transaction_kind) in [
        (
            ContractUpdateType::AddViaAuction,
            TransactionKind::AuctionDone,
        ),
        (ContractUpdateType::AddViaTrade, TransactionKind::Trade),
    ] {
        let player_id = league
            .add_veteran_player(&format!("Acquired via {add_kind:?}"))
            .await;
        let acquired = league
            .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
            .await;
        record_move(
            &league,
            league.team_id,
            RecordedMove {
                deadline_model: &week_1_lock,
                kind: transaction_kind,
                status: TeamUpdateStatus::Done,
                roster_contract_ids: vec![acquired.id],
                contract_moves: vec![(acquired.id, add_kind)],
            },
        )
        .await;

        let rejection = move_contract_to_ir(acquired, &week_1_lock, &league.db)
            .await
            .expect_err("an in-season add cannot go straight to IR");
        assert!(
            rejection.to_string().contains("straight to IR"),
            "unexpected rejection for {add_kind:?}: {rejection}"
        );
    }
}

/// Rules §10.3.1: the ban on direct-to-IR ends with the add's own week. A team that wins an
/// in-season auction and makes no other move still gets its player onto IR at the next lock.
#[tokio::test]
async fn a_quiet_team_can_ir_its_only_add_after_its_week_ends() {
    let Some(league) = weekly_moves_league("weekly_moves_quiet_team_ir").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    league
        .add_deadline(DeadlineKind::FreeAgentAuctionEnd, central(FA_AUCTION_END))
        .await;
    league
        .add_deadline(DeadlineKind::InSeasonRosterLock, central(WEEK_2_LOCK))
        .await;
    let in_season_lock = deadline_of(&league, DeadlineKind::InSeasonRosterLock).await;

    let player_id = league.add_veteran_player("Only signing").await;
    let signing = league
        .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
        .await;
    // The auction win's own committed update, i.e. the team's one and only roster move.
    record_move(
        &league,
        league.team_id,
        RecordedMove {
            deadline_model: &week_1_lock,
            kind: TransactionKind::AuctionDone,
            status: TeamUpdateStatus::Done,
            roster_contract_ids: vec![signing.id],
            contract_moves: vec![(signing.id, ContractUpdateType::AddViaAuction)],
        },
    )
    .await;

    let rejection = move_contract_to_ir(signing.clone(), &week_1_lock, &league.db)
        .await
        .expect_err("the add's own week still blocks IR");
    assert!(
        rejection.to_string().contains("straight to IR"),
        "unexpected rejection: {rejection}"
    );

    let ir_contract = move_contract_to_ir(signing, &in_season_lock, &league.db)
        .await
        .expect("the week the add was filed under has ended, so IR is open");
    assert!(ir_contract.is_ir);
}

/// Rules §10.3.1: once the contract has been committed to the active roster, IR is open to it.
#[tokio::test]
async fn a_contract_committed_to_the_active_roster_can_go_to_ir() {
    let Some(league) = weekly_moves_league("weekly_moves_committed_then_ir").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let player_id = league.add_veteran_player("Committed vet").await;
    let committed = league
        .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
        .await;
    let dropped = add_roster_contracts(&league, league.team_id, 1, "Dropped").await;

    // A drop made while already holding the vet: he is on the committed roster, and this is not his add.
    record_move(
        &league,
        league.team_id,
        RecordedMove {
            deadline_model: &week_1_lock,
            kind: TransactionKind::TeamUpdateDropContract,
            status: TeamUpdateStatus::Done,
            roster_contract_ids: vec![committed.id, dropped[0].id],
            contract_moves: vec![(dropped[0].id, ContractUpdateType::Drop)],
        },
    )
    .await;

    let ir_contract = move_contract_to_ir(committed, &week_1_lock, &league.db)
        .await
        .expect("a contract already committed to the active roster may move to IR in-season");
    assert!(ir_contract.is_ir);
}

#[tokio::test]
async fn a_traded_ir_contract_needs_room_at_its_new_team() {
    let Some(league) = weekly_moves_league("weekly_moves_traded_ir_contract").await else {
        return;
    };
    let preseason_lock = deadline_of(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let receiving_team_id = league.add_team("Receiving team").await;
    add_roster_contracts(
        &league,
        receiving_team_id,
        VET_OR_ROOKIE_LIMIT,
        "Receiving roster",
    )
    .await;

    let player_id = league.add_veteran_player("Injured vet").await;
    let contract_model = league
        .add_owned_contract(player_id, ContractKind::RookieExtension, 1, league.team_id)
        .await;
    let ir_contract = move_contract_to_ir(contract_model, &preseason_lock, &league.db)
        .await
        .expect("park the vet on IR before the season");

    let traded_contract =
        contract_queries::trade_contract_to_team(ir_contract, receiving_team_id, &league.db)
            .await
            .expect("trade the IR contract");

    assert!(
        !traded_contract.is_ir,
        "IR does not travel with a trade (rules §10.1.2)"
    );
    let violations = validate_league_rosters(&week_1_lock, &league.db)
        .await
        .expect("validate the receiving team's roster");
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.team_id == receiving_team_id)
            .map(|violation| violation.rule)
            .collect::<Vec<_>>(),
        vec![RosterRule::VeteranOrRookieLimit],
        "the traded contract counts against the 22-man limit at its new team"
    );
}

/// Rule 8.3.7's Mitchell/Alvarado example: once both of the week's adds are legally on the roster,
/// either of them may be dropped in the same week.
#[tokio::test]
async fn a_same_week_add_is_droppable_once_the_weeks_adds_fit_legally() {
    let Some(league) = weekly_moves_league("weekly_moves_drop_legal_same_week_add").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let adds = add_roster_contracts(&league, league.team_id, 2, "Auction win").await;
    for added_contract in &adds {
        record_auction_add(&league, added_contract, &week_1_lock).await;
    }

    let dropped = drop_contract_from_team(adds[0].clone(), &week_1_lock, &league.db)
        .await
        .expect("dropping a legally added same-week add is allowed");

    assert_eq!(
        dropped.team_id, None,
        "the dropped contract left the roster"
    );
}

/// The other half of rule 8.3.7: while the week's adds do not fit, dropping one of them to make
/// room for another is still rejected.
#[tokio::test]
async fn a_same_week_add_cannot_be_dropped_to_make_room_for_another_same_week_add() {
    let Some(league) = weekly_moves_league("weekly_moves_drop_same_week_add").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let roster = add_roster_contracts(
        &league,
        league.team_id,
        VET_OR_ROOKIE_LIMIT + 1,
        "Auction win",
    )
    .await;
    for added_contract in &roster[..2] {
        record_auction_add(&league, added_contract, &week_1_lock).await;
    }

    let rejection = drop_contract_from_team(roster[0].clone(), &week_1_lock, &league.db)
        .await
        .expect_err("dropping a same-week add off an illegal roster is rejected");

    assert!(
        rejection.to_string().contains("added this week"),
        "unexpected rejection: {rejection}"
    );
}

#[tokio::test]
async fn one_illegal_team_does_not_block_the_rest_of_the_league() {
    let Some(league) = weekly_moves_league("weekly_moves_per_team_lock").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let legal_team_id = league.add_team("Legal team").await;

    let illegal_roster = add_roster_contracts(
        &league,
        league.team_id,
        VET_OR_ROOKIE_LIMIT + 1,
        "Over the limit",
    )
    .await;
    let legal_roster = add_roster_contracts(&league, legal_team_id, 1, "Under the limit").await;
    let illegal_team_update = record_auction_add(&league, &illegal_roster[0], &week_1_lock).await;
    let legal_team_update = record_auction_add(&league, &legal_roster[0], &week_1_lock).await;

    let violations = lock_rosters(&week_1_lock, &league.db)
        .await
        .expect("lock rosters");

    assert_eq!(
        violations
            .iter()
            .map(|violation| (violation.team_id, violation.rule))
            .collect::<Vec<_>>(),
        vec![(league.team_id, RosterRule::VeteranOrRookieLimit)]
    );
    let persisted = roster_lock_violation_queries::find_violations_for_league(
        league.league_id,
        Some(week_1_lock.id),
        &league.db,
    )
    .await
    .expect("read the recorded violations");
    assert_eq!(
        persisted
            .iter()
            .map(|violation| (violation.team_id, violation.rule))
            .collect::<Vec<_>>(),
        vec![(league.team_id, RosterRule::VeteranOrRookieLimit)],
        "lock records the violations for the commissioner instead of only logging them"
    );

    assert_eq!(
        read_team_update_status(&league, legal_team_update.id).await,
        TeamUpdateStatus::Done,
        "the legal team's week locks"
    );
    assert_eq!(
        read_team_update_status(&league, illegal_team_update.id).await,
        TeamUpdateStatus::Pending,
        "the illegal team's week waits for the commissioner"
    );
}

/// Rules 13.1.1: an owner may re-order a week's moves however they like.
#[tokio::test]
async fn a_weeks_moves_keep_the_order_their_owner_chose() {
    let Some(league) = weekly_moves_league("weekly_move_order").await else {
        return;
    };
    let team_id = league.team_id;
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;

    let contracts = add_roster_contracts(&league, team_id, 3, "Ordered").await;
    let mut added_ids = Vec::with_capacity(contracts.len());
    for contract_model in &contracts {
        added_ids.push(
            record_auction_add(&league, contract_model, &week_1_lock)
                .await
                .id,
        );
    }

    team_update_queries::update_team_update_sequences(&added_ids[..2], &league.db)
        .await
        .expect("save the owner's order");

    let team_updates = team_update_queries::find_team_updates_by_team(
        team_id,
        None,
        Some(week_1_lock.id),
        &league.db,
    )
    .await
    .expect("read this week's moves");

    let sequence_of = |team_update_id: i64| {
        team_updates
            .iter()
            .find(|model| model.id == team_update_id)
            .expect("the moved update is still there")
            .sequence
    };
    assert_eq!(sequence_of(added_ids[0]), Some(0));
    assert_eq!(sequence_of(added_ids[1]), Some(1));
    assert_eq!(sequence_of(added_ids[2]), None);
}

async fn read_team_update_status(league: &TestLeague, team_update_id: i64) -> TeamUpdateStatus {
    team_update::Entity::find_by_id(team_update_id)
        .one(&league.db)
        .await
        .expect("read team update")
        .expect("team update exists")
        .status
}
