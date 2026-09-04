//! Reads/writes for the roster rules teams broke at a roster-lock deadline (rules §13.1.2, §13.2).

use std::fmt::Debug;

use color_eyre::{Result, eyre::bail};
use sea_orm::{ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use tracing::instrument;

use crate::{
    deadline,
    roster_lock_violation::{self, RosterRule},
};

/// One rule broken by one team's roster at lock time.
///
/// Rules §13.1.2/§13.2 send illegal rosters to the commissioner, so these are collected and
/// returned instead of raised as an error that would block the rest of the league. It is also the
/// insert shape: `replace_violations_for_teams` fills in the deadline and league.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRosterViolation {
    pub team_id: i64,
    pub rule: RosterRule,
    pub message: String,
}

/// Records the violations of the named teams at a deadline, replacing whatever an earlier run left.
///
/// `team_ids` is the scope being rewritten, not the teams that failed: an empty `violations` for a
/// team clears that team's rows, which is how a re-run after the owners fixed their rosters stops
/// reporting stale violations. Teams outside the scope keep the rows they have, so a single-team
/// caller cannot wipe the rest of the league.
#[instrument(skip(violations, db))]
pub async fn replace_violations_for_teams<C>(
    deadline_model: &deadline::Model,
    team_ids: &[i64],
    violations: &[TeamRosterViolation],
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    if let Some(out_of_scope) = violations
        .iter()
        .find(|violation| !team_ids.contains(&violation.team_id))
    {
        bail!(
            "Cannot record a violation for team id {} at deadline id {}: it sits outside the replaced scope, so a later run would never clear it.",
            out_of_scope.team_id,
            deadline_model.id
        );
    }

    roster_lock_violation::Entity::delete_many()
        .filter(roster_lock_violation::Column::DeadlineId.eq(deadline_model.id))
        .filter(roster_lock_violation::Column::TeamId.is_in(team_ids.iter().copied()))
        .exec(db)
        .await?;

    if violations.is_empty() {
        return Ok(());
    }

    let rows = violations
        .iter()
        .map(|violation| roster_lock_violation::ActiveModel {
            league_id: ActiveValue::Set(deadline_model.league_id),
            deadline_id: ActiveValue::Set(deadline_model.id),
            team_id: ActiveValue::Set(violation.team_id),
            rule: ActiveValue::Set(violation.rule),
            message: ActiveValue::Set(violation.message.clone()),
            ..Default::default()
        });
    roster_lock_violation::Entity::insert_many(rows)
        .exec(db)
        .await?;

    Ok(())
}

/// The league's recorded roster-lock violations, newest deadline first.
///
/// `deadline_id` narrows the read to one lock; without it the commissioner sees every lock the
/// league has run.
#[instrument(skip(db))]
pub async fn find_violations_for_league<C>(
    league_id: i64,
    deadline_id: Option<i64>,
    db: &C,
) -> Result<Vec<roster_lock_violation::Model>>
where
    C: ConnectionTrait,
{
    let mut query = roster_lock_violation::Entity::find()
        .filter(roster_lock_violation::Column::LeagueId.eq(league_id));
    if let Some(deadline_id) = deadline_id {
        query = query.filter(roster_lock_violation::Column::DeadlineId.eq(deadline_id));
    }

    let violation_models = query
        .order_by_desc(roster_lock_violation::Column::DeadlineId)
        .order_by_asc(roster_lock_violation::Column::TeamId)
        .order_by_asc(roster_lock_violation::Column::Id)
        .all(db)
        .await?;
    Ok(violation_models)
}
