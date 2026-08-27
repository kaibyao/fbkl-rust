use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, Order, QueryFilter, QueryOrder,
    prelude::DateTimeWithTimeZone,
};
use tracing::instrument;

use sea_orm::sea_query::Query;

use crate::{
    deadline::{self, DeadlineKind},
    job_run::{self, JobRunStatus},
};

/// Returns a league season's deadlines ordered by `date_time`, ties broken by `id`.
///
/// Insertion order (see `import-data` `import_deadlines`) encodes the intended processing order for
/// same-instant deadlines (e.g. a roster lock and a same-timestamp auction boundary), and `id`
/// is assigned in that order, so `(date_time, id)` reproduces it. Returning a `Vec` rather than a
/// datetime-keyed map is deliberate: a map silently drops one of two deadlines sharing a `date_time`.
#[instrument(skip(db))]
pub async fn find_sorted_deadlines_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<deadline::Model>>
where
    C: ConnectionTrait,
{
    let deadlines = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year)),
        )
        .order_by(deadline::Column::DateTime, Order::Asc)
        .order_by(deadline::Column::Id, Order::Asc)
        .all(db)
        .await?;

    Ok(deadlines)
}

#[instrument(skip(db))]
pub async fn find_deadline_by_id<C>(deadline_id: i64, db: &C) -> Result<deadline::Model>
where
    C: ConnectionTrait,
{
    deadline::Entity::find_by_id(deadline_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find a deadline with id = {}.", deadline_id))
}

#[instrument(skip(db))]
pub async fn find_deadline_for_season_by_type<C>(
    league_id: i64,
    end_of_season_year: i16,
    kind: DeadlineKind,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait,
{
    let maybe_deadline_model = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(deadline::Column::Kind.eq(kind)),
        )
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find a deadline for league (id = {}) and end-of-season year ({}) of type: {:?}.", league_id, end_of_season_year, kind))?;
    Ok(maybe_deadline_model)
}

/// The league season's first deadline at or after `datetime`, optionally narrowed to one kind.
///
/// Ordered like [`find_sorted_deadlines_for_league_season`], so "next" is the earliest match and
/// same-instant deadlines resolve in their intended processing order.
#[instrument(skip(db))]
pub async fn find_next_deadline_for_season_by_datetime<C>(
    league_id: i64,
    end_of_season_year: i16,
    datetime: DateTimeWithTimeZone,
    maybe_kind: Option<DeadlineKind>,
    db: &C,
) -> Result<Option<deadline::Model>>
where
    C: ConnectionTrait,
{
    let mut query = deadline::Entity::find().filter(
        deadline::Column::LeagueId
            .eq(league_id)
            .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year))
            .and(deadline::Column::DateTime.gte(datetime)),
    );
    if let Some(kind) = maybe_kind {
        query = query.filter(deadline::Column::Kind.eq(kind));
    }

    let maybe_deadline_model = query
        .order_by(deadline::Column::DateTime, Order::Asc)
        .order_by(deadline::Column::Id, Order::Asc)
        .one(db)
        .await?;
    Ok(maybe_deadline_model)
}

/// The lock a roster move made at `datetime` counts towards: the season's next one still to fire.
///
/// Spec 08 files a move under the lock it will be judged at, not the deadline that already passed,
/// so both the owner-facing mutations and server-side processing (an auction close) bucket their
/// transaction with this deadline. `None` once the season has no lock left to fire.
#[instrument(skip(db))]
pub async fn find_upcoming_roster_lock<C>(
    league_id: i64,
    end_of_season_year: i16,
    datetime: DateTimeWithTimeZone,
    db: &C,
) -> Result<Option<deadline::Model>>
where
    C: ConnectionTrait,
{
    let deadlines =
        find_sorted_deadlines_for_league_season(league_id, end_of_season_year, db).await?;

    Ok(deadlines
        .into_iter()
        .find(|candidate| candidate.date_time >= datetime && candidate.is_roster_lock()))
}

/// Finds deadlines (across all leagues) that are due at or before `now` and have no `Succeeded` job run.
///
/// I.e. work the scheduler still needs to dispatch. Ordered oldest
/// first so deadlines within a league season process in chronological order.
#[instrument(skip(db))]
pub async fn find_due_unprocessed_deadlines<C>(
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<Vec<deadline::Model>>
where
    C: ConnectionTrait,
{
    let deadlines = deadline::Entity::find()
        .filter(deadline::Column::DateTime.lte(now))
        .filter(
            deadline::Column::Id.not_in_subquery(
                Query::select()
                    .column(job_run::Column::DeadlineId)
                    .from(job_run::Entity)
                    .and_where(job_run::Column::DeadlineId.is_not_null())
                    .and_where(job_run::Column::Status.eq(JobRunStatus::Succeeded))
                    .to_owned(),
            ),
        )
        .order_by(deadline::Column::DateTime, Order::Asc)
        .all(db)
        .await?;

    Ok(deadlines)
}

#[instrument(skip(db))]
pub async fn find_most_recent_deadline_by_datetime<C>(
    league_id: i64,
    datetime: DateTimeWithTimeZone,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait,
{
    let maybe_deadline_model = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::DateTime.lte(datetime)),
        )
        .order_by(deadline::Column::DateTime, Order::Desc)
        .one(db)
        .await?
        .ok_or_else(|| {
            eyre!(
                "Could not find a recent deadline for league (id = {}) before or requal to: {}.",
                league_id,
                datetime.to_string()
            )
        })?;

    Ok(maybe_deadline_model)
}
