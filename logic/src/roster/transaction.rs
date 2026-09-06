//! Validates one transaction: the league unit of rules §13.1.4, a set of one team's moves in a
//! week that are applied and judged together. SQL transactions are called `db_txn` /
//! `DatabaseTransaction` everywhere in this workspace and are a different thing; the `db` handle
//! here may well be one.

use std::collections::{HashMap, HashSet};

use color_eyre::eyre::Result;
use fbkl_entity::{
    contract_queries,
    deadline::{self, DeadlineKind},
    deadline_queries,
    sea_orm::{ConnectionTrait, prelude::DateTimeWithTimeZone},
    team_update::{ContractUpdate, ContractUpdateType},
    team_update_queries::{
        TransactionStart, assign_team_updates_to_transaction, find_team_updates_after,
    },
};
use tracing::instrument;

use crate::{deadline_processing::roster_lock::validate_team_roster, roster::RosterMoveRejection};

/// Validates one transaction against rules §13.1.6: T1 roster legality, T2 no same-transaction
/// add-then-remove.
///
/// Call it after the transaction's moves are applied to the live rows inside a database
/// transaction, so `team_id`'s stored roster is the end state T1 asks about. An `Err` carrying a
/// `RosterMoveRejection` means the transaction is refused and the caller must return before
/// committing. T1 reads its limits from the period `transaction_datetime` falls in, which
/// [`find_governing_deadline`] resolves, not from the limits `deadline_model` will impose.
#[instrument(skip(db))]
pub async fn validate_transaction<C>(
    team_id: i64,
    transaction_updates: &[ContractUpdate],
    deadline_model: &deadline::Model,
    transaction_datetime: &DateTimeWithTimeZone,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    // T2 first: it costs at most one id lookup, against the whole rule sweep T1 runs.
    validate_no_add_then_remove(transaction_updates, deadline_model.kind, db).await?;

    let governing_deadline =
        find_governing_deadline(transaction_datetime, deadline_model, db).await?;
    let violations = validate_team_roster(team_id, &governing_deadline, db).await?;
    if !violations.is_empty() {
        return Err(RosterMoveRejection::TransactionLeavesRosterIllegal {
            team_id,
            violations,
        }
        .into());
    }

    Ok(())
}

/// Validates rules §13.1.6 T2 alone: a transaction may not remove a player it also acquired.
///
/// Split out of [`validate_transaction`] for `reorderTransactions`, which regroups moves that are
/// already applied and so has no roster state of its own to hand T1. `deadline_kind` is the kind of
/// the lock the moves are filed under, because rules §10.3.1 and §10.1.2 exempt the move to the IR
/// from T2 at the preseason lock.
#[instrument(skip(db))]
pub async fn validate_no_add_then_remove<C>(
    transaction_updates: &[ContractUpdate],
    deadline_kind: DeadlineKind,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    if let Some(offending_update) = find_same_transaction_add_then_remove(
        transaction_updates,
        &find_chain_roots(transaction_updates, deadline_kind, db).await?,
        deadline_kind,
    ) {
        return Err(RosterMoveRejection::SameTransactionAddThenRemove {
            contract_id: offending_update.contract_id,
            update_type: offending_update.update_type,
        }
        .into());
    }

    Ok(())
}

/// The contract-chain root each update's row belongs to, keyed by the row id the update names.
///
/// A drop or an IR move writes a replacement contract row, so the add and the removal of one
/// player inside a transaction never name the same `contract_id`. The chain root is what they do
/// share, which is why T2 cannot match on the update ids alone. Empty when the transaction holds
/// no add-and-removal pair to resolve, which is every single-move transaction.
async fn find_chain_roots<C>(
    transaction_updates: &[ContractUpdate],
    deadline_kind: DeadlineKind,
    db: &C,
) -> Result<HashMap<i64, i64>>
where
    C: ConnectionTrait,
{
    let has_add = transaction_updates
        .iter()
        .any(|update| is_add(update.update_type));
    let has_removal = transaction_updates
        .iter()
        .any(|update| is_removal(update.update_type, deadline_kind));
    if !has_add || !has_removal {
        return Ok(HashMap::new());
    }

    let contract_ids = transaction_updates
        .iter()
        .map(|update| update.contract_id)
        .collect();
    let chain_roots = contract_queries::find_contracts_by_ids(contract_ids, db)
        .await?
        .into_iter()
        .map(|contract_model| {
            (
                contract_model.id,
                contract_model
                    .original_contract_id
                    .unwrap_or(contract_model.id),
            )
        })
        .collect();

    Ok(chain_roots)
}

/// The update that breaks T2, i.e. the removal of a player this transaction also acquired.
///
/// `chain_roots` maps an update's `contract_id` to its chain root; an id it does not cover stands
/// for itself, which is what the add-only and removal-only transactions rely on.
fn find_same_transaction_add_then_remove<'updates>(
    transaction_updates: &'updates [ContractUpdate],
    chain_roots: &HashMap<i64, i64>,
    deadline_kind: DeadlineKind,
) -> Option<&'updates ContractUpdate> {
    let root_of = |contract_id: i64| {
        chain_roots
            .get(&contract_id)
            .copied()
            .unwrap_or(contract_id)
    };

    let acquired_roots: HashSet<i64> = transaction_updates
        .iter()
        .filter(|update| is_add(update.update_type))
        .map(|update| root_of(update.contract_id))
        .collect();

    transaction_updates.iter().find(|update| {
        is_removal(update.update_type, deadline_kind)
            && acquired_roots.contains(&root_of(update.contract_id))
    })
}

/// Numbers every move written since `transaction_start` as one transaction, returning what it did.
///
/// Read `transaction_start` with `find_transaction_start` before the first move is applied. Use
/// [`file_and_validate_transaction`] unless the caller judges the moves itself, as the roster lock
/// does for the wins nobody picked up.
#[instrument(skip(db))]
pub async fn file_transaction<C>(
    team_id: i64,
    deadline_model: &deadline::Model,
    transaction_start: &TransactionStart,
    db: &C,
) -> Result<Vec<ContractUpdate>>
where
    C: ConnectionTrait,
{
    let transaction_moves = find_team_updates_after(
        team_id,
        deadline_model.id,
        transaction_start.after_team_update_id,
        db,
    )
    .await?;

    let move_ids: Vec<i64> = transaction_moves
        .iter()
        .map(|team_update_model| team_update_model.id)
        .collect();
    assign_team_updates_to_transaction(transaction_start.transaction_number, &move_ids, db).await?;

    let mut transaction_updates = Vec::with_capacity(transaction_moves.len());
    for team_update_model in &transaction_moves {
        transaction_updates.extend(team_update_model.get_contract_updates()?);
    }

    Ok(transaction_updates)
}

/// Files every move written since `transaction_start` as one transaction, then judges it.
///
/// Both the numbering and the ruling run in the caller's database transaction, so an `Err` reaches
/// the caller before it commits and neither the moves nor their number persist. Read
/// `transaction_start` with `find_transaction_start` before the first move is applied.
/// `transaction_datetime` is when the moves were made, which picks the limits they are judged by.
#[instrument(skip(db))]
pub async fn file_and_validate_transaction<C>(
    team_id: i64,
    deadline_model: &deadline::Model,
    transaction_start: &TransactionStart,
    transaction_datetime: &DateTimeWithTimeZone,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let transaction_updates =
        file_transaction(team_id, deadline_model, transaction_start, db).await?;
    validate_transaction(
        team_id,
        &transaction_updates,
        deadline_model,
        transaction_datetime,
        db,
    )
    .await
}

/// The deadline whose roster rules govern a move made at `datetime` and filed under `upcoming_lock`.
///
/// A transaction is judged by the rules in force when it is made, not by the rules the lock it
/// counts towards will impose. `PreseasonFinalRosterLock` is the only lock in the whole preseason,
/// so reading its own limits would judge every preseason and offseason move at the 22/6/1 roster
/// and the $210 cap the league only holds teams to once that lock fires. Rules §5.1.2 allow 32
/// contracts through the preseason, §11.4.2 and §11.9.4 allow more rookie development contracts
/// than that when they are acquired by trade after season end, §4.2.1 holds the cap at $200 until
/// the veteran auction and rookie draft conclude, and §4.2.4 leaves the window from contract
/// advancement to the keeper deadline uncapped.
#[instrument(skip(db))]
pub async fn find_governing_deadline<C>(
    datetime: &DateTimeWithTimeZone,
    upcoming_lock: &deadline::Model,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait,
{
    if upcoming_lock.kind != DeadlineKind::PreseasonFinalRosterLock {
        return Ok(upcoming_lock.clone());
    }

    let is_before_keeper_deadline = deadline_queries::find_next_deadline_for_season_by_datetime(
        upcoming_lock.league_id,
        upcoming_lock.end_of_season_year,
        *datetime,
        Some(DeadlineKind::PreseasonKeeper),
        db,
    )
    .await?
    .is_some();
    let window_kind = if is_before_keeper_deadline {
        DeadlineKind::PreseasonStart
    } else {
        DeadlineKind::PreseasonRookieDraftStart
    };

    deadline_queries::find_deadline_for_season_by_type(
        upcoming_lock.league_id,
        upcoming_lock.end_of_season_year,
        window_kind,
        db,
    )
    .await
}

/// Whether the update type acquires a contract the team did not hold, i.e. T2's "acquired in a
/// transaction".
const fn is_add(update_type: ContractUpdateType) -> bool {
    matches!(
        update_type,
        ContractUpdateType::AddViaAuction
            | ContractUpdateType::AddViaTrade
            | ContractUpdateType::AddViaRookieDraft
    )
}

/// Whether the update type takes a contract off the counted roster, i.e. T2's "dropped or moved to
/// the IR".
///
/// Rules §10.3.1 and §10.1.2 exempt the move to the IR at the preseason lock, the one time a player
/// may go straight to the IR without being accommodated on the active roster first. A drop stays a
/// removal at every kind.
const fn is_removal(update_type: ContractUpdateType, deadline_kind: DeadlineKind) -> bool {
    match update_type {
        ContractUpdateType::Drop => true,
        ContractUpdateType::ToIR => {
            !matches!(deadline_kind, DeadlineKind::PreseasonFinalRosterLock)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(contract_id: i64, update_type: ContractUpdateType) -> ContractUpdate {
        ContractUpdate {
            contract_id,
            update_type,
            player_name_at_time: "Kelly Oubre".to_string(),
            player_team_abbr_at_time: "PHI".to_string(),
            player_team_name_at_time: "Philadelphia 76ers".to_string(),
        }
    }

    #[test]
    fn an_add_dropped_in_the_same_transaction_is_refused() {
        let updates = [
            update(1, ContractUpdateType::AddViaAuction),
            update(1, ContractUpdateType::Drop),
        ];

        let offending_update = find_same_transaction_add_then_remove(
            &updates,
            &HashMap::new(),
            DeadlineKind::InSeasonRosterLock,
        )
        .unwrap();

        assert_eq!(offending_update.contract_id, 1);
        assert_eq!(offending_update.update_type, ContractUpdateType::Drop);
    }

    #[test]
    fn an_add_sent_to_ir_in_the_same_transaction_is_refused() {
        let updates = [
            update(2, ContractUpdateType::AddViaTrade),
            update(2, ContractUpdateType::ToIR),
        ];

        let offending_update = find_same_transaction_add_then_remove(
            &updates,
            &HashMap::new(),
            DeadlineKind::InSeasonRosterLock,
        )
        .unwrap();

        assert_eq!(offending_update.update_type, ContractUpdateType::ToIR);
    }

    #[test]
    fn an_add_sent_to_ir_in_the_same_transaction_is_allowed_at_the_preseason_lock() {
        // Rules 10.3.1 and 10.1.2: the season start is the one time a player may go straight to IR.
        let updates = [
            update(2, ContractUpdateType::AddViaTrade),
            update(2, ContractUpdateType::ToIR),
        ];

        assert!(
            find_same_transaction_add_then_remove(
                &updates,
                &HashMap::new(),
                DeadlineKind::PreseasonFinalRosterLock
            )
            .is_none()
        );
    }

    #[test]
    fn an_add_dropped_in_the_same_transaction_is_refused_at_the_preseason_lock() {
        // Only the IR half of T2 is exempt at the season start; a drop is still a removal.
        let updates = [
            update(1, ContractUpdateType::AddViaAuction),
            update(1, ContractUpdateType::Drop),
        ];

        assert!(
            find_same_transaction_add_then_remove(
                &updates,
                &HashMap::new(),
                DeadlineKind::PreseasonFinalRosterLock
            )
            .is_some()
        );
    }

    #[test]
    fn a_rookie_draft_add_dropped_in_the_same_transaction_is_refused() {
        let updates = [
            update(3, ContractUpdateType::AddViaRookieDraft),
            update(3, ContractUpdateType::Drop),
        ];

        assert!(
            find_same_transaction_add_then_remove(
                &updates,
                &HashMap::new(),
                DeadlineKind::InSeasonRosterLock
            )
            .is_some()
        );
    }

    #[test]
    fn an_add_with_no_removal_is_allowed() {
        let updates = [
            update(4, ContractUpdateType::AddViaAuction),
            update(5, ContractUpdateType::AddViaAuction),
        ];

        assert!(
            find_same_transaction_add_then_remove(
                &updates,
                &HashMap::new(),
                DeadlineKind::InSeasonRosterLock
            )
            .is_none()
        );
    }

    #[test]
    fn an_add_moved_between_rd_and_rdi_in_the_same_transaction_is_allowed() {
        // T2 (rules 10.3.1) names dropping and moving to the IR; an RD<->RDI move is neither.
        for update_type in [ContractUpdateType::ToRdi, ContractUpdateType::FromRdi] {
            let updates = [
                update(3, ContractUpdateType::AddViaTrade),
                update(3, update_type),
            ];

            assert!(
                find_same_transaction_add_then_remove(
                    &updates,
                    &HashMap::new(),
                    DeadlineKind::InSeasonRosterLock
                )
                .is_none(),
                "{update_type:?} should not count as a removal"
            );
        }
    }

    #[test]
    fn a_drop_with_no_matching_add_is_allowed() {
        let updates = [update(6, ContractUpdateType::Drop)];

        assert!(
            find_same_transaction_add_then_remove(
                &updates,
                &HashMap::new(),
                DeadlineKind::InSeasonRosterLock
            )
            .is_none()
        );
    }

    #[test]
    fn dropping_a_different_contract_than_the_one_added_is_allowed() {
        let updates = [
            update(7, ContractUpdateType::AddViaTrade),
            update(8, ContractUpdateType::Drop),
        ];

        assert!(
            find_same_transaction_add_then_remove(
                &updates,
                &HashMap::new(),
                DeadlineKind::InSeasonRosterLock
            )
            .is_none()
        );
    }

    #[test]
    fn a_removal_naming_a_replacement_row_of_the_added_contract_is_refused() {
        // The drop writes contract row 91 to replace the added row 90; both trace back to root 41.
        let updates = [
            update(90, ContractUpdateType::AddViaTrade),
            update(91, ContractUpdateType::Drop),
        ];
        let chain_roots = HashMap::from([(90, 41), (91, 41)]);

        let offending_update = find_same_transaction_add_then_remove(
            &updates,
            &chain_roots,
            DeadlineKind::InSeasonRosterLock,
        )
        .unwrap();

        assert_eq!(offending_update.contract_id, 91);
    }

    #[test]
    fn a_removal_from_another_chain_is_allowed_when_roots_are_resolved() {
        let updates = [
            update(90, ContractUpdateType::AddViaTrade),
            update(91, ContractUpdateType::Drop),
        ];
        let chain_roots = HashMap::from([(90, 41), (91, 42)]);

        assert!(
            find_same_transaction_add_then_remove(
                &updates,
                &chain_roots,
                DeadlineKind::InSeasonRosterLock
            )
            .is_none()
        );
    }
}
