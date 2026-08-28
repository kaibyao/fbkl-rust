use std::collections::HashSet;

use color_eyre::eyre::Result;
use fbkl_entity::{
    deadline::{self},
    roster_lock_violation_queries::replace_violations_for_deadline,
    sea_orm::{ConnectionTrait, TransactionTrait},
    team_update::{self, TeamUpdateStatus},
    team_update_queries,
};
use tracing::instrument;

use super::{TeamRosterViolation, validate_league_rosters};

/// Lock every legal team's pending `team_updates` for the deadline.
///
/// Teams that broke a roster rule keep their `team_updates` Pending, and their broken rules are
/// recorded for the commissioner to read (rules 13.1.2/13.2) instead of blocking the rest of the
/// league. The recorded rows replace an earlier run's, so a re-run after fixes clears them.
#[instrument(skip(db))]
pub async fn lock_rosters<C>(
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<Vec<TeamRosterViolation>>
where
    C: ConnectionTrait + TransactionTrait,
{
    let violations = validate_league_rosters(deadline_model, db).await?;
    replace_violations_for_deadline(deadline_model, &violations, db).await?;
    let illegal_team_ids: HashSet<i64> = violations
        .iter()
        .map(|violation| violation.team_id)
        .collect();

    let deadline_team_updates =
        team_update_queries::find_team_updates_for_deadline(deadline_model, db).await?;
    team_update_queries::update_team_updates_with_status(
        lockable_team_update_ids(&deadline_team_updates, &illegal_team_ids),
        TeamUpdateStatus::Done,
        db,
    )
    .await?;

    // agents-allow-block: pre-existing commented-out draft-pick generation, kept verbatim.
    // if deadline_model.kind == DeadlineKind::PreseasonFinalRosterLock {
    //     // Propagate failure so the wrapping DB transaction rolls back and the scheduler
    //     // records a Failed job_run — swallowing this would silently skip pick generation.
    //     generate_future_draft_picks(
    //         deadline_model.league_id,
    //         deadline_model.end_of_season_year,
    //         db,
    //     )
    //     .await
    //     .wrap_err("Error generating future draft picks during final roster lock")?;
    // }

    Ok(violations)
}

/// Ids of the `team_updates` that may flip to Done: everything except the illegal teams' rows.
fn lockable_team_update_ids(
    team_update_models: &[team_update::Model],
    illegal_team_ids: &HashSet<i64>,
) -> Vec<i64> {
    team_update_models
        .iter()
        .filter(|team_update_model| !illegal_team_ids.contains(&team_update_model.team_id))
        .map(|team_update_model| team_update_model.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use fbkl_entity::sea_orm::prelude::{DateTimeWithTimeZone, Json};

    use super::*;

    fn team_update(id: i64, team_id: i64) -> team_update::Model {
        team_update::Model {
            id,
            data: Json::default(),
            effective_date: NaiveDate::from_ymd_opt(2024, 10, 22).unwrap(),
            sequence: None,
            status: TeamUpdateStatus::Pending,
            team_id,
            transaction_id: None,
            created_at: DateTimeWithTimeZone::default(),
            updated_at: DateTimeWithTimeZone::default(),
        }
    }

    #[test]
    fn one_illegal_team_does_not_block_its_siblings() {
        let team_updates = vec![team_update(1, 10), team_update(2, 11), team_update(3, 10)];
        let illegal_team_ids = HashSet::from([10]);

        assert_eq!(
            lockable_team_update_ids(&team_updates, &illegal_team_ids),
            vec![2]
        );
    }

    #[test]
    fn a_legal_league_locks_everything() {
        let team_updates = vec![team_update(1, 10), team_update(2, 11)];

        assert_eq!(
            lockable_team_update_ids(&team_updates, &HashSet::new()),
            vec![1, 2]
        );
    }
}
