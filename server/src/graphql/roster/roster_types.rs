//! GraphQL shapes for a team's week: its committed roster, its pending moves, and per-rule legality.

use async_graphql::{
    Enum, Error as GraphQlError, ErrorExtensions, InputObject, SimpleObject, Value,
    resolver_utils::enum_value, value,
};
use fbkl_entity::team_update;
use fbkl_logic::deadline_processing::roster_lock::{
    RosterRule as DomainRosterRule, TeamRosterViolation,
};

use super::super::{contract::Contract, team::TeamUpdate};
use crate::graphql::{ErrorCode, code_error};

/// The error a failing `legalizeRoster` returns: one entry per rule the roster breaks.
///
/// The entries go in a `violations` error extension, each naming the rule as its GraphQL enum
/// value, so a client can point at the rule it broke instead of parsing one joined message.
pub fn roster_illegal_error(violations: &[TeamRosterViolation]) -> GraphQlError {
    let payload = violations
        .iter()
        .map(|violation| {
            value!({
                "teamId": violation.team_id,
                "rule": enum_value(RosterRule::from(violation.rule)),
                "message": violation.message.clone(),
            })
        })
        .collect();

    code_error(ErrorCode::RosterIllegal)
        .extend_with(|_, extensions| extensions.set("violations", Value::List(payload)))
}

/// A roster rule that a team's roster can break at lock time.
///
/// Kept per-rule so callers can show a legality flag for each rule instead of one pass/fail.
// Mirrors the domain enum; the exhaustive `From` below keeps the two lists the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum RosterRule {
    IrSlots,
    PreseasonRosterLimit,
    RookieDevelopmentLimit,
    IntlRookieDevelopmentLimit,
    VeteranOrRookieLimit,
    SalaryCap,
}

impl From<DomainRosterRule> for RosterRule {
    fn from(rule: DomainRosterRule) -> Self {
        match rule {
            DomainRosterRule::IrSlots => Self::IrSlots,
            DomainRosterRule::PreseasonRosterLimit => Self::PreseasonRosterLimit,
            DomainRosterRule::RookieDevelopmentLimit => Self::RookieDevelopmentLimit,
            DomainRosterRule::IntlRookieDevelopmentLimit => Self::IntlRookieDevelopmentLimit,
            DomainRosterRule::VeteranOrRookieLimit => Self::VeteranOrRookieLimit,
            DomainRosterRule::SalaryCap => Self::SalaryCap,
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
    /// Puts one week's moves in the order the owner chose (rules §13.1.1).
    ///
    /// A move with no sequence was not placed by the owner, so it sorts after the placed ones in
    /// insertion order. Ordering is presentational: no roster rule reads it.
    pub fn in_owner_order(team_update_models: &[team_update::Model]) -> Vec<TeamUpdate> {
        let mut ordered: Vec<&team_update::Model> = team_update_models.iter().collect();
        ordered.sort_by_key(|model| (model.sequence.unwrap_or(i16::MAX), model.id));
        ordered.into_iter().map(TeamUpdate::from_model).collect()
    }

    /// Turns the league-wide violation list into one flag per rule for `team_id`.
    pub fn rule_legality_for_team(
        team_id: i64,
        violations: &[TeamRosterViolation],
    ) -> Vec<RosterRuleLegality> {
        DomainRosterRule::ALL
            .into_iter()
            .map(|rule| {
                let message = violations
                    .iter()
                    .find(|violation| violation.team_id == team_id && violation.rule == rule)
                    .map(|violation| violation.message.clone());
                RosterRuleLegality {
                    rule: rule.into(),
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
    use fbkl_entity::{
        sea_orm::prelude::{Date, DateTimeWithTimeZone, Json},
        team_update::TeamUpdateStatus,
    };

    use super::*;

    fn team_update_model(id: i64, sequence: Option<i16>) -> team_update::Model {
        team_update::Model {
            id,
            data: Json::default(),
            effective_date: Date::default(),
            sequence,
            status: TeamUpdateStatus::Pending,
            team_id: 1,
            transaction_id: None,
            created_at: DateTimeWithTimeZone::default(),
            updated_at: DateTimeWithTimeZone::default(),
        }
    }

    #[test]
    fn sequenced_moves_lead_and_unsequenced_ones_keep_insertion_order() {
        let models = [
            team_update_model(10, None),
            team_update_model(11, Some(1)),
            team_update_model(12, Some(0)),
            team_update_model(9, None),
        ];

        let ordered_ids: Vec<i64> = TeamWeek::in_owner_order(&models)
            .iter()
            .map(|team_update| team_update.id)
            .collect();

        assert_eq!(ordered_ids, vec![12, 11, 9, 10]);
    }

    fn violation(team_id: i64, rule: DomainRosterRule) -> TeamRosterViolation {
        TeamRosterViolation {
            team_id,
            rule,
            message: "over the limit".to_owned(),
        }
    }

    #[test]
    fn every_rule_gets_a_flag_and_only_the_broken_one_is_illegal() {
        let flags =
            TeamWeek::rule_legality_for_team(1, &[violation(1, DomainRosterRule::SalaryCap)]);

        assert_eq!(flags.len(), DomainRosterRule::ALL.len());
        let illegal: Vec<_> = flags.iter().filter(|flag| !flag.is_legal).collect();
        assert_eq!(illegal.len(), 1);
        assert_eq!(illegal[0].rule, RosterRule::SalaryCap);
        assert_eq!(illegal[0].message.as_deref(), Some("over the limit"));
    }

    #[test]
    fn the_illegal_roster_error_names_every_broken_rule() {
        let error = roster_illegal_error(&[violation(7, DomainRosterRule::VeteranOrRookieLimit)]);
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
        let flags = TeamWeek::rule_legality_for_team(1, &[violation(2, DomainRosterRule::IrSlots)]);

        assert!(flags.iter().all(|flag| flag.is_legal));
    }
}
