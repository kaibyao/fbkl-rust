use std::collections::HashSet;

use color_eyre::eyre::Result;
use fbkl_entity::{
    auction_queries::find_won_auctions_by_team,
    deadline::{self},
    roster_lock_violation_queries::replace_violations_for_teams,
    sea_orm::{ConnectionTrait, TransactionTrait},
    team_queries::find_teams_in_league,
    team_update::{self, TeamUpdateStatus},
    team_update_queries::{self, find_transaction_start, update_team_updates_with_status},
};
use tracing::instrument;

use super::{TeamRosterViolation, validate_league_rosters};
use crate::{auction::sign_won_auction, roster::file_transaction};

/// Lock every legal team's pending `team_updates` for the deadline.
///
/// Any auction win nobody picked up is signed first (rules §8.3.5), so the roster the sweep judges
/// is the one those wins leave.
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
    sign_unpicked_auction_wins(deadline_model, db).await?;

    let violations = validate_league_rosters(deadline_model, db).await?;
    // The sweep judged every team, so the whole league is the scope whose rows get rewritten.
    let league_team_ids: Vec<i64> = find_teams_in_league(deadline_model.league_id, db)
        .await?
        .into_iter()
        .map(|team_model| team_model.id)
        .collect();
    replace_violations_for_teams(deadline_model, &league_team_ids, &violations, db).await?;

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

/// Signs the auction wins nobody picked up, one no-drop transaction per team (rules §8.3.5).
///
/// A win an owner never submitted a pickup for cannot vanish, so the lock signs it and lets the
/// sweep above judge the roster it leaves. The signed rows go back to Pending because
/// `sign_won_auction` finishes the paths that run mid-week; at the lock, `lockable_team_update_ids`
/// is what decides whether a row is Done.
#[instrument(skip(db))]
async fn sign_unpicked_auction_wins<C>(deadline_model: &deadline::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait,
{
    let wins_by_team = find_won_auctions_by_team(
        deadline_model.league_id,
        deadline_model.end_of_season_year,
        db,
    )
    .await?;

    for (team_id, team_wins) in wins_by_team.iter_all() {
        let transaction_start = find_transaction_start(*team_id, deadline_model.id, db).await?;
        let mut signed_update_ids = Vec::with_capacity(team_wins.len());
        for (auction_model, winning_bid_model) in team_wins {
            let (_, team_update_model) = sign_won_auction(
                auction_model,
                winning_bid_model,
                deadline_model,
                Some(deadline_model.date_time.date_naive()),
                db,
            )
            .await?;
            signed_update_ids.push(team_update_model.id);
        }
        update_team_updates_with_status(signed_update_ids, TeamUpdateStatus::Pending, db).await?;
        file_transaction(*team_id, deadline_model, &transaction_start, db).await?;
    }

    Ok(())
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
            transaction_number: None,
            status: TeamUpdateStatus::Pending,
            team_id,
            league_event_id: None,
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
