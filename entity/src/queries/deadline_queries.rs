use std::{collections::HashMap, fmt::Debug};

use color_eyre::{eyre::eyre, Result};
use sea_orm::{
    prelude::DateTimeWithTimeZone, ColumnTrait, ConnectionTrait, EntityTrait, Order, QueryFilter,
    QueryOrder,
};
use tracing::instrument;

use crate::deadline::{self, DeadlineKind};

#[instrument]
pub async fn find_deadlines_by_date_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<HashMap<DateTimeWithTimeZone, deadline::Model>>
where
    C: ConnectionTrait + Debug,
{
    let deadlines = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year)),
        )
        .all(db)
        .await?;

    Ok(deadlines
        .into_iter()
        .map(|deadline_model| (deadline_model.date_time, deadline_model))
        .collect())
}

#[instrument]
pub async fn find_deadline_for_season_by_type<C>(
    league_id: i64,
    end_of_season_year: i16,
    kind: DeadlineKind,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait + Debug,
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

/// Attempts to retrieve the deadline immediately on or after the the given datetime within a league season.
#[instrument]
pub async fn find_next_deadline_for_season_by_datetime<C>(
    league_id: i64,
    end_of_season_year: i16,
    datetime: DateTimeWithTimeZone,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait + Debug,
{
    let maybe_deadline_model = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(deadline::Column::DateTime.gte(datetime)),
        )
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find a deadline for league (id = {}) and end-of-season year ({}) after: {}.", league_id, end_of_season_year, datetime.to_string()))?;
    Ok(maybe_deadline_model)
}

#[instrument]
pub async fn find_most_recent_deadline_by_datetime<C>(
    league_id: i64,
    datetime: DateTimeWithTimeZone,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait + Debug,
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
