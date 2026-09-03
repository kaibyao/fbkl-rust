//! Why a single-team roster move was refused by a league rule.
//!
//! The move fns in `ir`, `drop_contract`, and the rookie-development modules all return
//! `color_eyre::Result`, so a rule rejection and a database fault used to look the same to the
//! GraphQL resolver, which told the owner nothing beyond "roster move failed". Returning this type
//! lets the resolver downcast, give the rejection a client error code, and pass the rule message
//! through, the same way `BidRejection` works for auction bids.

use fbkl_entity::{
    contract::{ContractKind, ContractStatus},
    roster_lock_violation_queries::TeamRosterViolation,
    team_update::ContractUpdateType,
};

/// A roster move a league rule refuses. Each variant is a distinct user-facing rejection reason.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RosterMoveRejection {
    /// Rules §10.3.2: a contract already on IR cannot be moved there again.
    #[error("Contract {contract_id} is already in IR.")]
    AlreadyInIr { contract_id: i64 },
    /// Rules §10.3.2: activating from IR needs a contract that is on IR.
    #[error("Contract {contract_id} is not in IR, so it cannot be activated from IR.")]
    NotInIr { contract_id: i64 },
    /// A newer row in the contract's chain supersedes it, so the client is acting on a stale copy.
    #[error(
        "Contract {contract_id} is not the latest in its chain, so no roster move applies to it."
    )]
    NotLatestInChain { contract_id: i64 },
    /// Only a live contract can be moved; a replaced or expired row is a stale reference.
    #[error("Contract {contract_id} has status {status:?}, so no roster move applies to it.")]
    ContractNotActive {
        contract_id: i64,
        status: ContractStatus,
    },
    /// Rules §11.6: only a rookie-development contract can be activated as a rookie contract.
    #[error(
        "Contract {contract_id} is a {kind:?} contract, so it cannot be activated as a rookie contract."
    )]
    NotRookieDevelopment {
        contract_id: i64,
        kind: ContractKind,
    },
    /// Rules §13.1.6 (T2): a contract acquired in a transaction stays on the roster until a later
    /// transaction can remove it.
    #[error(
        "Contract {contract_id} was acquired in this transaction, so it cannot be {update_type:?} in the same transaction (rules §13.1.6). Drop a player the team already held, or make this move in a later transaction."
    )]
    SameTransactionAddThenRemove {
        contract_id: i64,
        update_type: ContractUpdateType,
    },
    /// Rules §13.1.6 (T1): the roster has to be legal once the transaction is applied.
    #[error(
        "This transaction leaves team {team_id}'s roster illegal, so none of it is applied.\n{}",
        joined_violation_messages(.violations)
    )]
    TransactionLeavesRosterIllegal {
        team_id: i64,
        /// One entry per rule the end state breaks, so a caller can report the rules rather than
        /// re-parse one joined message.
        violations: Vec<TeamRosterViolation>,
    },
    /// Rules §12.5.3: a trade's accommodating drop has to name a contract the submitting team
    /// holds once the trade's legs have applied, or there is nothing for it to remove.
    #[error(
        "Contract {contract_id} is not on team {team_id}'s roster once this trade applies, so it cannot be dropped with the trade."
    )]
    AccommodatingDropNotOnRoster { contract_id: i64, team_id: i64 },
    /// Rules §11.3.1: an RD↔RDI move needs the contract kind that move starts from.
    #[error(
        "Contract {contract_id} is a {kind:?} contract, but this move requires a {expected:?} contract."
    )]
    WrongContractKind {
        contract_id: i64,
        kind: ContractKind,
        expected: ContractKind,
    },
}

/// The broken rules as the owner reads them: one message per line.
fn joined_violation_messages(violations: &[TeamRosterViolation]) -> String {
    violations
        .iter()
        .map(|violation| violation.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
