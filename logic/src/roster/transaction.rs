//! Validates one transaction: the league unit of rules §13.1.4, a set of one team's moves in a
//! week that are applied and judged together. SQL transactions are called `db_txn` /
//! `DatabaseTransaction` everywhere in this workspace and are a different thing; the `db` handle
//! here may well be one.

use std::collections::{HashMap, HashSet};

use color_eyre::eyre::Result;
use fbkl_entity::{
    contract_queries, deadline,
    sea_orm::ConnectionTrait,
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
/// committing.
#[instrument(skip(db))]
pub async fn validate_transaction<C>(
    team_id: i64,
    transaction_updates: &[ContractUpdate],
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    // T2 first: it costs at most one id lookup, against the whole rule sweep T1 runs.
    if let Some(offending_update) = find_same_transaction_add_then_remove(
        transaction_updates,
        &find_chain_roots(transaction_updates, db).await?,
    ) {
        return Err(RosterMoveRejection::SameTransactionAddThenRemove {
            contract_id: offending_update.contract_id,
            update_type: offending_update.update_type,
        }
        .into());
    }

    let violations = validate_team_roster(team_id, deadline_model, db).await?;
    if !violations.is_empty() {
        return Err(RosterMoveRejection::TransactionLeavesRosterIllegal {
            team_id,
            violations,
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
        .any(|update| is_removal(update.update_type));
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
        is_removal(update.update_type) && acquired_roots.contains(&root_of(update.contract_id))
    })
}

/// Files every move written since `transaction_start` as one transaction, then judges it.
///
/// Both the numbering and the ruling run in the caller's database transaction, so an `Err` reaches
/// the caller before it commits and neither the moves nor their number persist. Read
/// `transaction_start` with `find_transaction_start` before the first move is applied.
#[instrument(skip(db))]
pub async fn file_and_validate_transaction<C>(
    team_id: i64,
    deadline_model: &deadline::Model,
    transaction_start: &TransactionStart,
    db: &C,
) -> Result<()>
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

    validate_transaction(team_id, &transaction_updates, deadline_model, db).await
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
const fn is_removal(update_type: ContractUpdateType) -> bool {
    matches!(
        update_type,
        ContractUpdateType::Drop | ContractUpdateType::ToIR
    )
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

        let offending_update =
            find_same_transaction_add_then_remove(&updates, &HashMap::new()).unwrap();

        assert_eq!(offending_update.contract_id, 1);
        assert_eq!(offending_update.update_type, ContractUpdateType::Drop);
    }

    #[test]
    fn an_add_sent_to_ir_in_the_same_transaction_is_refused() {
        let updates = [
            update(2, ContractUpdateType::AddViaTrade),
            update(2, ContractUpdateType::ToIR),
        ];

        let offending_update =
            find_same_transaction_add_then_remove(&updates, &HashMap::new()).unwrap();

        assert_eq!(offending_update.update_type, ContractUpdateType::ToIR);
    }

    #[test]
    fn a_rookie_draft_add_dropped_in_the_same_transaction_is_refused() {
        let updates = [
            update(3, ContractUpdateType::AddViaRookieDraft),
            update(3, ContractUpdateType::Drop),
        ];

        assert!(find_same_transaction_add_then_remove(&updates, &HashMap::new()).is_some());
    }

    #[test]
    fn an_add_with_no_removal_is_allowed() {
        let updates = [
            update(4, ContractUpdateType::AddViaAuction),
            update(5, ContractUpdateType::AddViaAuction),
        ];

        assert!(find_same_transaction_add_then_remove(&updates, &HashMap::new()).is_none());
    }

    #[test]
    fn a_drop_with_no_matching_add_is_allowed() {
        let updates = [update(6, ContractUpdateType::Drop)];

        assert!(find_same_transaction_add_then_remove(&updates, &HashMap::new()).is_none());
    }

    #[test]
    fn dropping_a_different_contract_than_the_one_added_is_allowed() {
        let updates = [
            update(7, ContractUpdateType::AddViaTrade),
            update(8, ContractUpdateType::Drop),
        ];

        assert!(find_same_transaction_add_then_remove(&updates, &HashMap::new()).is_none());
    }

    #[test]
    fn a_removal_naming_a_replacement_row_of_the_added_contract_is_refused() {
        // The drop writes contract row 91 to replace the added row 90; both trace back to root 41.
        let updates = [
            update(90, ContractUpdateType::AddViaTrade),
            update(91, ContractUpdateType::Drop),
        ];
        let chain_roots = HashMap::from([(90, 41), (91, 41)]);

        let offending_update =
            find_same_transaction_add_then_remove(&updates, &chain_roots).unwrap();

        assert_eq!(offending_update.contract_id, 91);
    }

    #[test]
    fn a_removal_from_another_chain_is_allowed_when_roots_are_resolved() {
        let updates = [
            update(90, ContractUpdateType::AddViaTrade),
            update(91, ContractUpdateType::Drop),
        ];
        let chain_roots = HashMap::from([(90, 41), (91, 42)]);

        assert!(find_same_transaction_add_then_remove(&updates, &chain_roots).is_none());
    }
}
