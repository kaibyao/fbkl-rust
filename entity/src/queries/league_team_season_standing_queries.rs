//! Reads/writes for the frozen per-season standings inputs the rookie draft consumes (rules §7.2).

use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    sea_query::OnConflict,
};
use tracing::instrument;

use crate::league_team_season_standing;

/// One team's standings inputs as entered by the commissioner.
#[derive(Clone, Copy, Debug)]
pub struct NewLeagueTeamSeasonStanding {
    pub team_id: i64,
    pub regular_season_rank: i16,
    pub mid_season_rank: i16,
    pub made_playoffs: bool,
    pub playoff_finish: Option<i16>,
}

/// Every team's standings row for the season, best final rank first.
#[instrument]
pub async fn find_standings_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<league_team_season_standing::Model>>
where
    C: ConnectionTrait + Debug,
{
    let standing_models = league_team_season_standing::Entity::find()
        .filter(league_team_season_standing::Column::LeagueId.eq(league_id))
        .filter(league_team_season_standing::Column::EndOfSeasonYear.eq(end_of_season_year))
        .order_by_asc(league_team_season_standing::Column::RegularSeasonRank)
        .all(db)
        .await?;
    Ok(standing_models)
}

/// Upserts the season's standings rows, replacing any prior entry for the same team.
///
/// Re-entry is allowed until the draft starts (the commissioner may fix a typo); once the lottery
/// has run the draft order is already persisted, so later edits do not move picks.
#[instrument(skip(rows))]
pub async fn upsert_standings_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    rows: Vec<NewLeagueTeamSeasonStanding>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    if rows.is_empty() {
        return Ok(());
    }

    let models_to_insert = rows
        .into_iter()
        .map(|row| league_team_season_standing::ActiveModel {
            id: ActiveValue::NotSet,
            league_id: ActiveValue::Set(league_id),
            team_id: ActiveValue::Set(row.team_id),
            end_of_season_year: ActiveValue::Set(end_of_season_year),
            regular_season_rank: ActiveValue::Set(row.regular_season_rank),
            mid_season_rank: ActiveValue::Set(row.mid_season_rank),
            made_playoffs: ActiveValue::Set(row.made_playoffs),
            playoff_finish: ActiveValue::Set(row.playoff_finish),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        });

    league_team_season_standing::Entity::insert_many(models_to_insert)
        .on_conflict(
            OnConflict::columns([
                league_team_season_standing::Column::LeagueId,
                league_team_season_standing::Column::EndOfSeasonYear,
                league_team_season_standing::Column::TeamId,
            ])
            .update_columns([
                league_team_season_standing::Column::RegularSeasonRank,
                league_team_season_standing::Column::MidSeasonRank,
                league_team_season_standing::Column::MadePlayoffs,
                league_team_season_standing::Column::PlayoffFinish,
            ])
            .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(())
}
