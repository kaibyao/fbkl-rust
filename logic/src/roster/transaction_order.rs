//! Judges a proposed transaction order for one week against rules §13.1.6 T1.
//!
//! Reordering a week never changes the roster it ends with, but it does change the roster part way
//! through: a team at the contract limit that recorded a drop and then a pickup ends the week legal
//! either way, yet the reversed order holds 23 contracts once its first transaction applies. Rules
//! §13.1.1 let an owner reorder freely, and the commissioner ruled that T1 still has to hold after
//! every transaction of whatever order is saved. The database holds only the week's end state, so
//! this replays the week's contract changes over the roster the week opened with.

use std::collections::HashMap;

use color_eyre::eyre::Result;
use fbkl_entity::{
    contract, contract_queries, deadline,
    sea_orm::ConnectionTrait,
    team_update::{ContractUpdate, ContractUpdateType},
};
use tracing::instrument;

use crate::{
    deadline_processing::roster_lock::validate_roster_contracts, roster::RosterMoveRejection,
};

/// Refuses a proposed transaction order that leaves the roster illegal after any of its
/// transactions (rules §13.1.6 T1).
///
/// `week_updates` is every contract change the week recorded, oldest move first, which is the order
/// they were applied in. It has to be the week's changes and no others: `team_id`'s live rows are
/// the week's end state, and the roster the week opened with comes from undoing that list.
/// `ordered_transactions` is the order being saved, one entry per transaction.
///
/// `governing_deadline` supplies the limits, the same one [`validate_transaction`] judges a single
/// transaction with. An `Err` carrying [`RosterMoveRejection::TransactionLeavesRosterIllegal`]
/// carries the violations of the first transaction that breaks a rule, so the caller returns before
/// committing.
///
/// [`validate_transaction`]: crate::roster::validate_transaction
#[instrument(skip(db))]
pub async fn validate_transaction_order<C>(
    team_id: i64,
    week_updates: &[ContractUpdate],
    ordered_transactions: &[Vec<ContractUpdate>],
    governing_deadline: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let contracts_by_id = load_changed_contracts(week_updates, db).await?;
    let mut roster = opening_roster(team_id, week_updates, &contracts_by_id, db).await?;

    // ponytail: one row per chain, so a chain's moves listed out of order judge a state it never held; sort per chain if saveable.
    for transaction_updates in ordered_transactions {
        for update in transaction_updates {
            if let Some(change) = roster_change(update, team_id, &contracts_by_id) {
                set_roster_slot(&mut roster, change.root, change.after);
            }
        }

        let team_contracts: Vec<contract::Model> = roster.values().cloned().collect();
        let violations =
            validate_roster_contracts(team_id, &team_contracts, governing_deadline, db).await?;
        if !violations.is_empty() {
            return Err(RosterMoveRejection::TransactionLeavesRosterIllegal {
                team_id,
                violations,
            }
            .into());
        }
    }

    Ok(())
}

/// The roster the week opened with: the live rows with every change in `week_updates` undone.
async fn opening_roster<C>(
    team_id: i64,
    week_updates: &[ContractUpdate],
    contracts_by_id: &HashMap<i64, contract::Model>,
    db: &C,
) -> Result<HashMap<i64, contract::Model>>
where
    C: ConnectionTrait,
{
    let mut roster: HashMap<i64, contract::Model> =
        contract_queries::find_active_contracts_for_team(team_id, db)
            .await?
            .into_iter()
            .map(|contract_model| (root_of(&contract_model), contract_model))
            .collect();

    for update in week_updates.iter().rev() {
        if let Some(change) = roster_change(update, team_id, contracts_by_id) {
            set_roster_slot(&mut roster, change.root, change.before);
        }
    }

    Ok(roster)
}

/// Every contract row the week's changes name, plus the row each of them replaced.
async fn load_changed_contracts<C>(
    week_updates: &[ContractUpdate],
    db: &C,
) -> Result<HashMap<i64, contract::Model>>
where
    C: ConnectionTrait,
{
    let changed_rows = contract_queries::find_contracts_by_ids(
        week_updates
            .iter()
            .map(|update| update.contract_id)
            .collect(),
        db,
    )
    .await?;
    // A second query, because a row's predecessor is only known once the row itself is read.
    let previous_rows = contract_queries::find_contracts_by_ids(
        changed_rows
            .iter()
            .filter_map(|contract_model| contract_model.previous_contract_id)
            .collect(),
        db,
    )
    .await?;

    Ok(changed_rows
        .into_iter()
        .chain(previous_rows)
        .map(|contract_model| (contract_model.id, contract_model))
        .collect())
}

/// What one contract change does to the team's roster.
struct RosterChange<'contracts> {
    /// The contract chain the change touches. A roster holds one row per chain: the player's
    /// current one.
    root: i64,
    /// The row the chain held before the change; `None` when the change acquired the player.
    before: Option<&'contracts contract::Model>,
    /// The row the chain holds after it; `None` when the change took the player off the roster.
    after: Option<&'contracts contract::Model>,
}

/// Reads a recorded change as the pair of rows it swaps on the roster.
///
/// A trade's `TradedAway` names the row the team held, because the row the trade wrote belongs to
/// the other team. Every other change names the row it wrote, so the row it replaced is that row's
/// predecessor. Either row counts only while `team_id` holds it, which is what tells a pickup from
/// a move between squads.
fn roster_change<'contracts>(
    update: &ContractUpdate,
    team_id: i64,
    contracts_by_id: &'contracts HashMap<i64, contract::Model>,
) -> Option<RosterChange<'contracts>> {
    let changed = contracts_by_id.get(&update.contract_id)?;
    let root = root_of(changed);

    if update.update_type == ContractUpdateType::TradedAway {
        return Some(RosterChange {
            root,
            before: Some(changed),
            after: None,
        });
    }

    Some(RosterChange {
        root,
        before: changed
            .previous_contract_id
            .and_then(|previous_id| contracts_by_id.get(&previous_id))
            .filter(|contract_model| contract_model.team_id == Some(team_id)),
        after: Some(changed).filter(|contract_model| contract_model.team_id == Some(team_id)),
    })
}

fn set_roster_slot(
    roster: &mut HashMap<i64, contract::Model>,
    root: i64,
    row: Option<&contract::Model>,
) {
    if let Some(contract_model) = row {
        roster.insert(root, contract_model.clone());
    } else {
        roster.remove(&root);
    }
}

fn root_of(contract_model: &contract::Model) -> i64 {
    contract_model
        .original_contract_id
        .unwrap_or(contract_model.id)
}

#[cfg(test)]
mod tests {
    use fbkl_entity::contract::{ContractKind, ContractStatus};

    use super::*;

    const TEAM_ID: i64 = 5;

    fn row(id: i64, previous_contract_id: Option<i64>, team_id: Option<i64>) -> contract::Model {
        contract::Model {
            id,
            year_number: 1,
            kind: ContractKind::Veteran,
            is_ir: false,
            salary: 1,
            end_of_season_year: 2026,
            status: ContractStatus::Active,
            league_id: 1,
            league_player_id: None,
            player_id: Some(1),
            previous_contract_id,
            original_contract_id: Some(previous_contract_id.unwrap_or(id)),
            team_id,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        }
    }

    fn update(contract_id: i64, update_type: ContractUpdateType) -> ContractUpdate {
        ContractUpdate {
            contract_id,
            update_type,
            player_name_at_time: "Kelly Oubre".to_owned(),
            player_team_abbr_at_time: "PHI".to_owned(),
            player_team_name_at_time: "Philadelphia 76ers".to_owned(),
        }
    }

    fn contracts(rows: Vec<contract::Model>) -> HashMap<i64, contract::Model> {
        rows.into_iter()
            .map(|contract_model| (contract_model.id, contract_model))
            .collect()
    }

    #[test]
    fn a_drop_takes_the_row_the_team_held_off_the_roster() {
        let by_id = contracts(vec![
            row(100, None, Some(TEAM_ID)),
            row(101, Some(100), None),
        ]);

        let change =
            roster_change(&update(101, ContractUpdateType::Drop), TEAM_ID, &by_id).unwrap();

        assert_eq!(change.root, 100);
        assert_eq!(change.before.map(|c| c.id), Some(100));
        assert!(change.after.is_none());
    }

    #[test]
    fn a_pickup_puts_a_row_on_a_chain_the_team_did_not_hold() {
        let by_id = contracts(vec![row(100, None, Some(TEAM_ID))]);

        let change = roster_change(
            &update(100, ContractUpdateType::AddViaAuction),
            TEAM_ID,
            &by_id,
        )
        .unwrap();

        assert_eq!(change.root, 100);
        assert!(change.before.is_none());
        assert_eq!(change.after.map(|c| c.id), Some(100));
    }

    #[test]
    fn a_move_to_ir_swaps_one_row_of_the_chain_for_the_next() {
        let by_id = contracts(vec![
            row(100, None, Some(TEAM_ID)),
            row(101, Some(100), Some(TEAM_ID)),
        ]);

        let change =
            roster_change(&update(101, ContractUpdateType::ToIR), TEAM_ID, &by_id).unwrap();

        assert_eq!(change.before.map(|c| c.id), Some(100));
        assert_eq!(change.after.map(|c| c.id), Some(101));
    }

    #[test]
    fn a_trade_away_names_the_row_the_team_held_not_the_one_it_wrote() {
        // TradedAway names the row this roster held, so its successor belongs to the other team.
        let by_id = contracts(vec![row(100, None, Some(TEAM_ID))]);

        let change = roster_change(
            &update(100, ContractUpdateType::TradedAway),
            TEAM_ID,
            &by_id,
        )
        .unwrap();

        assert_eq!(change.before.map(|c| c.id), Some(100));
        assert!(change.after.is_none());
    }

    #[test]
    fn an_add_via_trade_ignores_the_row_the_other_team_held() {
        let by_id = contracts(vec![
            row(100, None, Some(9)),
            row(101, Some(100), Some(TEAM_ID)),
        ]);

        let change = roster_change(
            &update(101, ContractUpdateType::AddViaTrade),
            TEAM_ID,
            &by_id,
        )
        .unwrap();

        assert!(change.before.is_none());
        assert_eq!(change.after.map(|c| c.id), Some(101));
    }
}
