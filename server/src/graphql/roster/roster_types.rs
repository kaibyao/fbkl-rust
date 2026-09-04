//! GraphQL shapes for a team's week: its committed roster, its pending moves, and per-rule legality.

use async_graphql::{
    Enum, Error as GraphQlError, ErrorExtensions, InputObject, SimpleObject, Value,
    resolver_utils::enum_value, value,
};
use fbkl_entity::{roster_lock_violation, team_update};
use fbkl_logic::deadline_processing::roster_lock::{RosterRule, TeamRosterViolation};

use super::super::{contract::Contract, team::TeamUpdate};
use crate::graphql::{ErrorCode, code_error};

/// The error a transaction refused by T1 returns: one entry per rule its end state breaks.
///
/// The entries go in a `violations` error extension, each naming the rule as its GraphQL enum
/// value, so a client can point at the rule it broke instead of parsing one joined message.
pub fn roster_illegal_error(violations: &[TeamRosterViolation]) -> GraphQlError {
    let payload = violations
        .iter()
        .map(|violation| {
            value!({
                "teamId": violation.team_id,
                "rule": enum_value(violation.rule),
                "message": violation.message.clone(),
            })
        })
        .collect();

    code_error(ErrorCode::RosterIllegal)
        .extend_with(|_, extensions| extensions.set("violations", Value::List(payload)))
}

/// A team's recorded roster-lock failure: one rule it broke at one deadline (rules §13.1.2, §13.2).
#[derive(SimpleObject)]
pub struct RosterLockViolation {
    pub id: i64,
    pub deadline_id: i64,
    pub team_id: i64,
    pub rule: RosterRule,
    pub message: String,
}

impl RosterLockViolation {
    /// Maps a persisted row to its GraphQL shape; `league_id` is already known from the request.
    pub fn from_model(violation_model: roster_lock_violation::Model) -> Self {
        Self {
            id: violation_model.id,
            deadline_id: violation_model.deadline_id,
            team_id: violation_model.team_id,
            rule: violation_model.rule,
            message: violation_model.message,
        }
    }
}

/// Whether one roster rule holds for a team, with the failure text when it does not.
#[derive(SimpleObject)]
pub struct RosterRuleLegality {
    pub rule: RosterRule,
    pub is_legal: bool,
    pub message: Option<String>,
}

/// One transaction of a team's week: the moves it applied and that T1 and T2 judge together
/// (rules §13.1.4).
#[derive(SimpleObject)]
pub struct TeamTransaction {
    /// The stored transaction number, or `None` for a move no order has placed yet.
    pub transaction_number: Option<i16>,
    pub moves: Vec<TeamUpdate>,
}

/// A team's roster as it stands for one deadline, plus every move recorded for that week.
#[derive(SimpleObject)]
pub struct TeamWeek {
    pub team_id: i64,
    pub deadline_id: i64,
    pub contracts: Vec<Contract>,
    /// The week's moves whatever their status, grouped as `reorderTransactions` takes them.
    ///
    /// Drops, trades and auction wins are Done as soon as they are recorded, but rules §13.1.1
    /// order covers the whole week, so a Pending-only list could not be reordered. Each move
    /// carries its own `status` for a client that wants only the pending ones.
    pub transactions: Vec<TeamTransaction>,
    pub rule_legality: Vec<RosterRuleLegality>,
    pub is_legal: bool,
}

impl TeamWeek {
    /// Groups one week's moves into the transactions the owner chose, in their order (§13.1.1).
    ///
    /// Moves sharing a transaction number are one transaction. A move with no number was not
    /// placed by the owner, so it is a transaction of its own and sorts after the placed ones in
    /// insertion order.
    pub fn in_owner_order(team_update_models: &[team_update::Model]) -> Vec<TeamTransaction> {
        let mut ordered: Vec<&team_update::Model> = team_update_models.iter().collect();
        ordered.sort_by_key(|model| (model.transaction_number.unwrap_or(i16::MAX), model.id));

        let mut transactions: Vec<TeamTransaction> = Vec::new();
        for model in ordered {
            match transactions.last_mut() {
                Some(last)
                    if last.transaction_number.is_some()
                        && last.transaction_number == model.transaction_number =>
                {
                    last.moves.push(TeamUpdate::from_model(model));
                }
                _ => transactions.push(TeamTransaction {
                    transaction_number: model.transaction_number,
                    moves: vec![TeamUpdate::from_model(model)],
                }),
            }
        }
        transactions
    }

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

/// The roster moves a transaction can carry.
///
/// No add variants: adds reach a transaction through the trade and FA-pickup paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum RosterMoveKind {
    Drop,
    MoveToIr,
    ActivateFromIr,
    ActivateRookie,
}

/// One move in a `submitTransaction` batch.
#[derive(InputObject)]
pub struct RosterMove {
    pub contract_id: i64,
    pub kind: RosterMoveKind,
}

#[cfg(test)]
mod tests {
    use fbkl_entity::{
        sea_orm::prelude::{Date, DateTimeWithTimeZone, Json},
        team_update::TeamUpdateStatus,
    };

    use super::*;

    fn team_update_model(id: i64, transaction_number: Option<i16>) -> team_update::Model {
        team_update::Model {
            id,
            data: Json::default(),
            effective_date: Date::default(),
            transaction_number,
            status: TeamUpdateStatus::Pending,
            team_id: 1,
            league_event_id: None,
            created_at: DateTimeWithTimeZone::default(),
            updated_at: DateTimeWithTimeZone::default(),
        }
    }

    #[test]
    fn moves_group_by_transaction_and_unnumbered_ones_trail_one_per_transaction() {
        let models = [
            team_update_model(10, None),
            team_update_model(11, Some(1)),
            team_update_model(12, Some(0)),
            team_update_model(13, Some(1)),
            team_update_model(9, None),
        ];

        let grouped: Vec<(Option<i16>, Vec<i64>)> = TeamWeek::in_owner_order(&models)
            .iter()
            .map(|transaction| {
                (
                    transaction.transaction_number,
                    transaction.moves.iter().map(|model| model.id).collect(),
                )
            })
            .collect();

        assert_eq!(
            grouped,
            vec![
                (Some(0), vec![12]),
                (Some(1), vec![11, 13]),
                (None, vec![9]),
                (None, vec![10]),
            ]
        );
    }

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

        let illegal: Vec<_> = flags.iter().filter(|flag| !flag.is_legal).collect();
        assert_eq!(illegal.len(), 1);
        assert_eq!(illegal[0].rule, RosterRule::SalaryCap);
        assert_eq!(illegal[0].message.as_deref(), Some("over the limit"));
    }

    #[test]
    fn the_illegal_roster_error_names_every_broken_rule() {
        let error = roster_illegal_error(&[violation(7, RosterRule::VeteranOrRookieLimit)]);
        let extensions = error.extensions.expect("the error carries extensions");

        assert_eq!(extensions.get("code"), Some(&"ROSTER_ILLEGAL".into()));
        assert_eq!(
            extensions.get("violations"),
            Some(&Value::List(vec![value!({
                "teamId": 7,
                "rule": "VETERAN_OR_ROOKIE_LIMIT",
                "message": "over the limit",
            })]))
        );
    }

    #[test]
    fn another_teams_violation_does_not_mark_this_team_illegal() {
        let flags = TeamWeek::rule_legality_for_team(1, &[violation(2, RosterRule::IrSlots)]);

        assert!(flags.iter().all(|flag| flag.is_legal));
    }
}
