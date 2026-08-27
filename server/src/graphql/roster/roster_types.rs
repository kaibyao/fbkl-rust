//! GraphQL shapes for a team's week: its committed roster, its pending moves, and per-rule legality.

use async_graphql::{Enum, InputObject, SimpleObject};
use fbkl_logic::deadline_processing::roster_lock::{RosterRule, TeamRosterViolation};

use super::super::{contract::Contract, team::TeamUpdate};

/// Whether one roster rule holds for a team, with the failure text when it does not.
#[derive(SimpleObject)]
pub struct RosterRuleLegality {
    pub rule: RosterRule,
    pub is_legal: bool,
    pub message: Option<String>,
}

/// A team's roster as it stands for one deadline, plus the moves still pending for that week.
#[derive(SimpleObject)]
pub struct TeamWeek {
    pub team_id: i64,
    pub deadline_id: i64,
    pub contracts: Vec<Contract>,
    pub pending_moves: Vec<TeamUpdate>,
    pub rule_legality: Vec<RosterRuleLegality>,
    pub is_legal: bool,
}

impl TeamWeek {
    /// Turns the league-wide violation list into one flag per rule for `team_id`.
    pub fn rule_legality_for_team(
        team_id: i64,
        violations: &[TeamRosterViolation],
    ) -> Vec<RosterRuleLegality> {
        RosterRule::ALL
            .into_iter()
            .map(|rule| {
                let message = violations
                    .iter()
                    .find(|violation| violation.team_id == team_id && violation.rule == rule)
                    .map(|violation| violation.message.clone());
                RosterRuleLegality {
                    rule,
                    is_legal: message.is_none(),
                    message,
                }
            })
            .collect()
    }
}

/// The roster moves the season-start wizard can batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum RosterMoveKind {
    Drop,
    MoveToIr,
    ActivateFromIr,
    ActivateRookie,
}

/// One move in a `legalizeRoster` batch.
#[derive(InputObject)]
pub struct RosterMove {
    pub contract_id: i64,
    pub kind: RosterMoveKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violation(team_id: i64, rule: RosterRule) -> TeamRosterViolation {
        TeamRosterViolation {
            team_id,
            rule,
            message: "over the limit".to_owned(),
        }
    }

    #[test]
    fn every_rule_gets_a_flag_and_only_the_broken_one_is_illegal() {
        let flags = TeamWeek::rule_legality_for_team(1, &[violation(1, RosterRule::SalaryCap)]);

        assert_eq!(flags.len(), RosterRule::ALL.len());
        let illegal: Vec<_> = flags.iter().filter(|flag| !flag.is_legal).collect();
        assert_eq!(illegal.len(), 1);
        assert_eq!(illegal[0].rule, RosterRule::SalaryCap);
        assert_eq!(illegal[0].message.as_deref(), Some("over the limit"));
    }

    #[test]
    fn another_teams_violation_does_not_mark_this_team_illegal() {
        let flags = TeamWeek::rule_legality_for_team(1, &[violation(2, RosterRule::IrSlots)]);

        assert!(flags.iter().all(|flag| flag.is_legal));
    }
}
