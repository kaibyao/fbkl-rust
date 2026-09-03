//! Cover for the weekly-moves rules that only hold across several moves at once. The judged unit is
//! the transaction: a set of one team's moves in a week, applied and judged together (rules
//! §13.1.4). T1 says the roster must be legal after each transaction; T2 says a contract acquired in
//! a transaction may not be dropped, or moved to the IR, in that same transaction (rules §13.1.6,
//! which is the whole of §8.3.7 and of §10.3.1). The roster lock is the last check of the week
//! rather than the only one (rules §13.1.2).
//!
//! Each test seeds its own league with a preseason final roster lock and a week 1 roster lock, so
//! "in-season" and "preseason" can both be exercised without a full season's deadlines.
//!
//! The commissioner's two worked weeks are fixtures here, because a real week is better evidence
//! than a synthetic roster: Steve's 2015-02-22 (two trade transactions, a contract received in the
//! first and dropped in the second) and Kai's 2021-11-01 (six transactions, free-agent adds dropped
//! by a later trade).

use color_eyre::eyre::{Report, Result, eyre};
use fbkl_entity::{
    contract::{self, ContractKind, ContractStatus},
    contract_queries,
    deadline::{self, DeadlineKind},
    deadline_queries,
    league_event::{self, LeagueEventKind},
    league_event_queries, roster_lock_violation_queries,
    sea_orm::{ActiveValue, EntityTrait},
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries,
};
use fbkl_logic::{
    deadline_processing::{RosterRule, TeamRosterViolation, lock_rosters, validate_league_rosters},
    drop_contract::drop_contract_from_team,
    ir::move_contract_to_ir,
    roster::{RosterMoveRejection, validate_transaction},
};
use fbkl_test_support::{TestLeague, central};

const END_OF_SEASON_YEAR: i16 = 2026;
/// Rules §11.2: a roster carries at most 22 veteran or rookie-scale contracts.
const VET_OR_ROOKIE_LIMIT: usize = 22;
const PRESEASON_LOCK: &str = "2025-10-20T18:00:00";
const WEEK_1_LOCK: &str = "2025-10-27T18:00:00";

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
/// Rookie extensions rather than veteran contracts; both count the same against the 22-man limit.
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
    kind: LeagueEventKind,
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

    let league_event_model = league_event_queries::insert_league_event(
        league_event::ActiveModel {
            end_of_season_year: ActiveValue::Set(END_OF_SEASON_YEAR),
            kind: ActiveValue::Set(kind),
            league_id: ActiveValue::Set(league.league_id),
            deadline_id: ActiveValue::Set(deadline_model.id),
            ..Default::default()
        },
        &league.db,
    )
    .await
    .expect("insert league_event");

    let contract_updates = contract_moves
        .into_iter()
        .map(|(contract_id, update_type)| contract_update(contract_id, update_type))
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
            league_event_id: ActiveValue::Set(Some(league_event_model.id)),
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
            kind: LeagueEventKind::AuctionDone,
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

/// The move a `team_update` records. The player fields carry display text no validator reads.
const fn contract_update(contract_id: i64, update_type: ContractUpdateType) -> ContractUpdate {
    ContractUpdate {
        contract_id,
        update_type,
        player_name_at_time: String::new(),
        player_team_abbr_at_time: String::new(),
        player_team_name_at_time: String::new(),
    }
}

/// One move inside a transaction, i.e. a contract row change and how the owner made it.
enum Move {
    /// A free-agent or auction win joining the roster (`AddViaAuction`).
    Win(contract::Model),
    /// A contract arriving from another team (`AddViaTrade`).
    TradeFor(contract::Model),
    /// A contract leaving for the given team (`TradedAway`); T1 counts what it leaves behind.
    TradeAway(contract::Model, i64),
    /// A drop, one of T2's two removals.
    Drop(contract::Model),
    /// A move to the IR, T2's other removal.
    ToIr(contract::Model),
}

/// The live row of `contract_model`'s chain, i.e. what a move has to act on once an earlier move
/// replaced the row the fixture named. Every roster move writes a replacement row, so a week that
/// touches one player twice names a stale row the second time, and the row a fixture holds cannot
/// be trusted to still be the live one.
async fn live_row(league: &TestLeague, contract_model: contract::Model) -> Result<contract::Model> {
    contract_queries::find_contract_chain(contract_model.id, &league.db)
        .await?
        .into_iter()
        .find(|chain_row| chain_row.status == ContractStatus::Active)
        .ok_or_else(|| eyre!("contract chain {} has no live row", contract_model.id))
}

/// Applies `moves` to `team_id`'s live rows and judges them as one transaction (rules §13.1.6),
/// i.e. the shape every transaction submission path shares.
///
/// A win joins the roster through the same chain replacement a trade uses; how the contract arrived
/// is what its `ContractUpdate` records, and that is all T2 reads. Nothing rolls back on rejection,
/// because these tests assert on the rejection rather than on what the caller would have kept.
async fn submit_transaction(
    league: &TestLeague,
    team_id: i64,
    deadline_model: &deadline::Model,
    moves: Vec<Move>,
) -> Result<()> {
    let mut contract_updates = Vec::with_capacity(moves.len());

    for move_to_apply in moves {
        let (update_contract_id, update_type) = match move_to_apply {
            Move::Win(named_contract_model) => {
                let joining = live_row(league, named_contract_model).await?;
                let joined =
                    contract_queries::trade_contract_to_team(joining, team_id, &league.db).await?;
                (joined.id, ContractUpdateType::AddViaAuction)
            }
            Move::TradeFor(named_contract_model) => {
                let arriving = live_row(league, named_contract_model).await?;
                let arrived =
                    contract_queries::trade_contract_to_team(arriving, team_id, &league.db).await?;
                (arrived.id, ContractUpdateType::AddViaTrade)
            }
            Move::TradeAway(named_contract_model, to_team_id) => {
                let leaving = live_row(league, named_contract_model).await?;
                // The sending side records the row it gave up, so this id is the pre-trade one.
                let given_up_contract_id = leaving.id;
                contract_queries::trade_contract_to_team(leaving, to_team_id, &league.db).await?;
                (given_up_contract_id, ContractUpdateType::TradedAway)
            }
            Move::Drop(named_contract_model) => {
                let to_drop = live_row(league, named_contract_model).await?;
                let dropped = drop_contract_from_team(to_drop, deadline_model, &league.db).await?;
                (dropped.id, ContractUpdateType::Drop)
            }
            Move::ToIr(named_contract_model) => {
                let to_park = live_row(league, named_contract_model).await?;
                let on_ir = move_contract_to_ir(to_park, deadline_model, &league.db).await?;
                (on_ir.id, ContractUpdateType::ToIR)
            }
        };
        contract_updates.push(contract_update(update_contract_id, update_type));
    }

    validate_transaction(team_id, &contract_updates, deadline_model, &league.db).await
}

/// The rule rejection behind a refused transaction, i.e. what the GraphQL resolver downcasts to.
fn rejection(error: &Report) -> &RosterMoveRejection {
    error
        .downcast_ref::<RosterMoveRejection>()
        .unwrap_or_else(|| panic!("expected a rule rejection, got: {error}"))
}

/// The team's counted contracts, i.e. the non-IR veteran or rookie-scale rows rule §11.2 caps at 22.
async fn counted_contracts(league: &TestLeague, team_id: i64) -> usize {
    contract_queries::find_active_contracts_for_team(team_id, &league.db)
        .await
        .expect("read the team's contracts")
        .iter()
        .filter(|contract_model| {
            !contract_model.is_ir
                && matches!(
                    contract_model.kind,
                    ContractKind::Rookie | ContractKind::RookieExtension | ContractKind::Veteran
                )
        })
        .count()
}

/// One named rookie-scale contract, on `team_id` when it is `Some`, in the free-agent pool when not.
async fn named_contract(
    league: &TestLeague,
    name: &str,
    kind: ContractKind,
    owner_team_id: Option<i64>,
) -> contract::Model {
    let player_id = league.add_veteran_player(name).await;
    match owner_team_id {
        Some(team_id) => league.add_owned_contract(player_id, kind, 1, team_id).await,
        None => league.add_unowned_contract(player_id, kind, 1).await,
    }
}

/// The named contracts of a fixture's roster or trade package, in the order given.
async fn named_contracts(
    league: &TestLeague,
    names: &[&str],
    owner_team_id: Option<i64>,
) -> Vec<contract::Model> {
    let mut contracts = Vec::with_capacity(names.len());
    for name in names {
        contracts
            .push(named_contract(league, name, ContractKind::RookieExtension, owner_team_id).await);
    }
    contracts
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

/// Rules §10.1.2 and §13.1.6: a move to IR is judged by the transaction it belongs to, so the move
/// itself is open at any lock. T2 (nothing acquired in a transaction goes to IR in that same
/// transaction) is `validate_transaction`'s job and is unit-tested in `fbkl-logic`.
#[tokio::test]
async fn a_move_to_ir_is_open_at_any_lock() {
    let Some(league) = weekly_moves_league("weekly_moves_direct_to_ir").await else {
        return;
    };
    let preseason_lock = deadline_of(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;

    let preseason_player_id = league.add_veteran_player("Preseason signing").await;
    let preseason_signing = league
        .add_owned_contract(
            preseason_player_id,
            ContractKind::RookieExtension,
            1,
            league.team_id,
        )
        .await;
    let preseason_ir = move_contract_to_ir(preseason_signing, &preseason_lock, &league.db)
        .await
        .expect("the preseason final roster lock allows direct-to-IR");
    assert!(preseason_ir.is_ir);

    let in_season_player_id = league.add_veteran_player("In-season signing").await;
    let in_season_signing = league
        .add_owned_contract(
            in_season_player_id,
            ContractKind::RookieExtension,
            1,
            league.team_id,
        )
        .await;
    // The auction win's own committed update, i.e. what used to block this move to IR.
    record_auction_add(&league, &in_season_signing, &week_1_lock).await;

    let in_season_ir = move_contract_to_ir(in_season_signing, &week_1_lock, &league.db)
        .await
        .expect("an in-season move to IR is judged by its transaction, not by this fn");
    assert!(in_season_ir.is_ir);

    let already_on_ir = move_contract_to_ir(in_season_ir, &week_1_lock, &league.db)
        .await
        .expect_err("a contract already on IR cannot go there again");
    assert!(
        already_on_ir.to_string().contains("already in IR"),
        "unexpected rejection: {already_on_ir}"
    );
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

/// Rules §13.1.6 (T2): a contract acquired in one transaction may be dropped in a later one. This
/// is the shape the commissioner approved for Neto, and the shape the deleted drop-time gate
/// refused.
#[tokio::test]
async fn an_add_dropped_in_a_later_transaction_is_allowed() {
    let Some(league) = weekly_moves_league("weekly_moves_drop_in_a_later_transaction").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    add_roster_contracts(&league, league.team_id, 20, "Holdover").await;
    let wins = named_contracts(&league, &["Won first", "Won second"], None).await;

    submit_transaction(
        &league,
        league.team_id,
        &week_1_lock,
        wins.iter().cloned().map(Move::Win).collect(),
    )
    .await
    .expect("the week's two wins fit on the roster");
    assert_eq!(counted_contracts(&league, league.team_id).await, 22);

    submit_transaction(
        &league,
        league.team_id,
        &week_1_lock,
        vec![Move::Drop(wins[0].clone())],
    )
    .await
    .expect("a later transaction may drop what an earlier one added");

    assert_eq!(counted_contracts(&league, league.team_id).await, 21);
}

/// Rules §10.3.1, restated as T2: a contract acquired in a transaction cannot be parked on the IR by
/// that same transaction. It may go there in a later one.
#[tokio::test]
async fn an_add_sent_to_ir_in_its_own_transaction_is_refused() {
    let Some(league) = weekly_moves_league("weekly_moves_add_then_ir").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    add_roster_contracts(&league, league.team_id, 21, "Holdover").await;
    let trade_partner_team_id = league.add_team("Trade partner").await;
    let arrival = named_contract(
        &league,
        "Injured arrival",
        ContractKind::RookieExtension,
        Some(trade_partner_team_id),
    )
    .await;

    let refusal = submit_transaction(
        &league,
        league.team_id,
        &week_1_lock,
        vec![Move::TradeFor(arrival.clone()), Move::ToIr(arrival)],
    )
    .await
    .expect_err("T2 refuses a move to the IR in the transaction that acquired the contract");

    assert!(
        matches!(
            rejection(&refusal),
            RosterMoveRejection::SameTransactionAddThenRemove {
                update_type: ContractUpdateType::ToIR,
                ..
            }
        ),
        "unexpected rejection: {refusal}"
    );
}

/// Rule §8.3.7's own Mitchell/Alvarado example: an owner at 21 of 22 wins both, then tries to fit
/// Mitchell by dropping Alvarado. A week's free-agent adds are one transaction, so that puts an add
/// and its drop in the same transaction and T2 refuses it.
///
/// The end state would hold 22 counted contracts, so no roster-count rule is broken. T2 is the only
/// reason this fails, which is what separates it from a plain over-the-limit week.
#[tokio::test]
async fn the_weeks_free_agent_adds_cannot_pay_for_each_other() {
    let Some(league) = weekly_moves_league("weekly_moves_fa_adds_pay_for_each_other").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    add_roster_contracts(&league, league.team_id, 21, "Holdover").await;
    let wins = named_contracts(&league, &["Donovan Mitchell", "Jose Alvarado"], None).await;
    let [mitchell, alvarado]: [contract::Model; 2] = wins.try_into().expect("two free agent wins");

    let refusal = submit_transaction(
        &league,
        league.team_id,
        &week_1_lock,
        vec![
            Move::Win(mitchell),
            Move::Win(alvarado.clone()),
            Move::Drop(alvarado),
        ],
    )
    .await
    .expect_err("T2 refuses dropping one of the week's own adds to fit another");

    assert!(
        matches!(
            rejection(&refusal),
            RosterMoveRejection::SameTransactionAddThenRemove {
                update_type: ContractUpdateType::Drop,
                ..
            }
        ),
        "the reason is T2, not a roster count: {refusal}"
    );
    assert!(
        validate_league_rosters(&week_1_lock, &league.db)
            .await
            .expect("validate the roster the refused transaction left behind")
            .is_empty(),
        "the end state itself is legal, so only T2 can refuse this transaction"
    );
}

/// Steve's 2015-02-22 week, the week the historical import used to abort on. Two trade transactions;
/// Carmelo Anthony arrives in the first and is dropped in the second, and both end at 22 counted
/// contracts. Reggie Bullock and Tony Snell are rookie-development contracts, which do not count
/// toward the 22, and Paul George sits on the IR, which does not count either.
#[allow(clippy::too_many_lines)] // a real week's roster and packages, spelled out; splitting hurts readability
#[tokio::test]
async fn steves_2015_02_22_week_is_legal_transaction_by_transaction() {
    let Some(league) = weekly_moves_league("weekly_moves_steve_2015_02_22").await else {
        return;
    };
    let preseason_lock = deadline_of(&league, DeadlineKind::PreseasonFinalRosterLock).await;
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let steve_team_id = league.team_id;

    let outgoing = named_contracts(
        &league,
        &[
            "John Wall",
            "Monta Ellis",
            "Tyreke Evans",
            "Anderson Varejao",
            "Taj Gibson",
            "Greg Smith",
        ],
        Some(steve_team_id),
    )
    .await;
    let [wall, ellis, evans, varejao, gibson, smith]: [contract::Model; 6] =
        outgoing.try_into().expect("six named holdovers");
    add_roster_contracts(&league, steve_team_id, 16, "Holdover").await;
    let bullock = named_contract(
        &league,
        "Reggie Bullock",
        ContractKind::RookieDevelopment,
        Some(steve_team_id),
    )
    .await;
    let george = named_contract(
        &league,
        "Paul George",
        ContractKind::RookieExtension,
        Some(steve_team_id),
    )
    .await;
    move_contract_to_ir(george, &preseason_lock, &league.db)
        .await
        .expect("park Paul George on the IR before the season");
    assert_eq!(counted_contracts(&league, steve_team_id).await, 22);

    let larry_kevin_team_id = league.add_team("Larry-Kevin").await;
    let first_package = named_contracts(
        &league,
        &["Carmelo Anthony", "Jonas Jerebko"],
        Some(larry_kevin_team_id),
    )
    .await;
    let [carmelo, jerebko]: [contract::Model; 2] =
        first_package.try_into().expect("two incoming contracts");

    submit_transaction(
        &league,
        steve_team_id,
        &week_1_lock,
        vec![
            Move::TradeAway(wall, larry_kevin_team_id),
            Move::TradeFor(carmelo.clone()),
            Move::TradeFor(jerebko),
            Move::Drop(varejao),
        ],
    )
    .await
    .expect("the Larry-Kevin trade plus its accommodating drop is legal");
    assert_eq!(counted_contracts(&league, steve_team_id).await, 22);

    let mike_yu_peter_team_id = league.add_team("MikeYu-Peter").await;
    let second_package = named_contracts(
        &league,
        &[
            "Jordan Farmar",
            "Brian Roberts",
            "Chris Kaman",
            "Brandon Jennings",
            "Nick Young",
        ],
        Some(mike_yu_peter_team_id),
    )
    .await;
    let snell = named_contract(
        &league,
        "Tony Snell",
        ContractKind::RookieDevelopment,
        Some(mike_yu_peter_team_id),
    )
    .await;

    let mut second_trade_moves = vec![
        Move::TradeAway(ellis, mike_yu_peter_team_id),
        Move::TradeAway(evans, mike_yu_peter_team_id),
        Move::TradeAway(bullock, mike_yu_peter_team_id),
    ];
    second_trade_moves.extend(second_package.into_iter().map(Move::TradeFor));
    second_trade_moves.push(Move::TradeFor(snell));
    second_trade_moves.push(Move::Drop(carmelo));
    second_trade_moves.push(Move::Drop(gibson));
    second_trade_moves.push(Move::Drop(smith));

    submit_transaction(&league, steve_team_id, &week_1_lock, second_trade_moves)
        .await
        .expect("the MikeYu-Peter trade may drop the contract the first trade brought in");

    assert_eq!(
        counted_contracts(&league, steve_team_id).await,
        22,
        "the week ends at 22 counted contracts, with Carmelo dropped"
    );
    let steve_contracts =
        contract_queries::find_active_contracts_for_team(steve_team_id, &league.db)
            .await
            .expect("read the week's end state");
    assert!(
        !steve_contracts
            .iter()
            .any(|contract_model| contract_model.kind == ContractKind::FreeAgent),
        "a dropped contract leaves the roster"
    );
}

/// Kai's 2021-11-01 week, the week the commissioner ruled on. Six transactions: Campazzo arrives in
/// the second and is dropped by the fourth, and Neto and Reaves are won in the fourth and dropped by
/// the fifth. Every transaction is legal on its own.
#[allow(clippy::too_many_lines)] // a real week's roster and packages, spelled out; splitting hurts readability
#[tokio::test]
async fn kais_2021_11_01_week_is_legal_transaction_by_transaction() {
    let Some(league) = weekly_moves_league("weekly_moves_kai_2021_11_01").await else {
        return;
    };
    let week_1_lock = deadline_of(&league, DeadlineKind::Week1RosterLock).await;
    let kai_team_id = league.team_id;

    let holdovers = named_contracts(
        &league,
        &[
            "Giannis Antetokounmpo",
            "Derrick Jones Jr",
            "Taurean Prince",
            "Alize Johnson",
            "Cory Joseph",
        ],
        Some(kai_team_id),
    )
    .await;
    let [giannis, jones_jr, prince, alize_johnson, joseph]: [contract::Model; 5] =
        holdovers.try_into().expect("five named holdovers");
    add_roster_contracts(&league, kai_team_id, 16, "Holdover").await;
    assert_eq!(counted_contracts(&league, kai_team_id).await, 21);

    let partner_team_id = league.add_team("Trade partner").await;
    let incoming = named_contracts(
        &league,
        &[
            "Devin Booker",
            "Facundo Campazzo",
            "Terence Davis",
            "Goran Dragic",
            "Quentin Grimes",
        ],
        Some(partner_team_id),
    )
    .await;
    let [booker, campazzo, terence_davis, dragic, grimes]: [contract::Model; 5] =
        incoming.try_into().expect("five incoming contracts");
    let free_agents = named_contracts(
        &league,
        &[
            "Raul Neto",
            "Isaiah Hartenstein",
            "Austin Reaves",
            "Damion Lee",
        ],
        None,
    )
    .await;
    let [neto, hartenstein, reaves, lee]: [contract::Model; 4] =
        free_agents.try_into().expect("four free agent wins");

    submit_transaction(
        &league,
        kai_team_id,
        &week_1_lock,
        vec![
            Move::TradeAway(giannis, partner_team_id),
            Move::TradeFor(booker),
        ],
    )
    .await
    .expect("T1: Giannis for Booker");
    assert_eq!(counted_contracts(&league, kai_team_id).await, 21);

    submit_transaction(
        &league,
        kai_team_id,
        &week_1_lock,
        vec![Move::TradeFor(campazzo.clone())],
    )
    .await
    .expect("T2: trade for Campazzo");
    assert_eq!(counted_contracts(&league, kai_team_id).await, 22);

    submit_transaction(
        &league,
        kai_team_id,
        &week_1_lock,
        vec![
            Move::TradeAway(jones_jr, partner_team_id),
            Move::TradeFor(terence_davis),
        ],
    )
    .await
    .expect("T3: Jones Jr for Terence Davis");
    assert_eq!(counted_contracts(&league, kai_team_id).await, 22);

    submit_transaction(
        &league,
        kai_team_id,
        &week_1_lock,
        vec![
            Move::Win(neto.clone()),
            Move::Win(hartenstein),
            Move::Win(reaves.clone()),
            Move::Win(lee),
            Move::Drop(prince),
            Move::Drop(alize_johnson),
            Move::Drop(joseph),
            Move::Drop(campazzo),
        ],
    )
    .await
    .expect("T4: the week's free agent adds, paid for by holdovers and by T2's Campazzo");
    assert_eq!(counted_contracts(&league, kai_team_id).await, 22);

    submit_transaction(
        &league,
        kai_team_id,
        &week_1_lock,
        vec![
            Move::TradeFor(dragic.clone()),
            Move::TradeFor(grimes),
            Move::Drop(reaves),
            Move::Drop(neto),
        ],
    )
    .await
    .expect("T5: a later transaction may drop Reaves and Neto, won in T4");
    assert_eq!(counted_contracts(&league, kai_team_id).await, 22);

    submit_transaction(
        &league,
        kai_team_id,
        &week_1_lock,
        vec![Move::TradeAway(dragic, partner_team_id)],
    )
    .await
    .expect("T6: trade Dragic away");

    assert_eq!(
        counted_contracts(&league, kai_team_id).await,
        21,
        "the week ends at 21 counted contracts, with Neto and Reaves dropped"
    );
    assert!(
        lock_rosters(&week_1_lock, &league.db)
            .await
            .expect("lock the week")
            .iter()
            .all(|violation| violation.team_id != kai_team_id),
        "the whole week locks clean"
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

    team_update_queries::update_team_update_transaction_numbers(
        &[vec![added_ids[0]], vec![added_ids[1]]],
        &league.db,
    )
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

    let transaction_number_of = |team_update_id: i64| {
        team_updates
            .iter()
            .find(|model| model.id == team_update_id)
            .expect("the moved update is still there")
            .transaction_number
    };
    assert_eq!(transaction_number_of(added_ids[0]), Some(0));
    assert_eq!(transaction_number_of(added_ids[1]), Some(1));
    assert_eq!(transaction_number_of(added_ids[2]), None);
}

async fn read_team_update_status(league: &TestLeague, team_update_id: i64) -> TeamUpdateStatus {
    team_update::Entity::find_by_id(team_update_id)
        .one(&league.db)
        .await
        .expect("read team update")
        .expect("team update exists")
        .status
}
