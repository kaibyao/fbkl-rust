//! Reads/writes for the rookie draft lottery audit rows (rules §7.2.4-§7.2.5).
//!
//! The seed row is written before the draw happens (commit-reveal) and the drawn slots are written
//! after, so a stored lottery is proof the order was not re-rolled.

use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    sea_query::OnConflict,
};
use tracing::instrument;

use crate::{rookie_draft_lottery, rookie_draft_lottery_pick};

/// One drawn first-round slot, with the ball count that won it.
#[derive(Clone, Copy, Debug)]
pub struct NewRookieDraftLotteryPick {
    pub pick_number: i16,
    pub team_id: i64,
    pub balls_held: i16,
}

/// The league season's lottery row, if one has been created.
#[instrument(skip(db))]
pub async fn find_lottery_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Option<rookie_draft_lottery::Model>>
where
    C: ConnectionTrait,
{
    let lottery_model = rookie_draft_lottery::Entity::find()
        .filter(rookie_draft_lottery::Column::LeagueId.eq(league_id))
        .filter(rookie_draft_lottery::Column::EndOfSeasonYear.eq(end_of_season_year))
        .one(db)
        .await?;
    Ok(lottery_model)
}

/// The drawn slots for a lottery, pick 1 first.
#[instrument(skip(db))]
pub async fn find_lottery_picks<C>(
    rookie_draft_lottery_id: i64,
    db: &C,
) -> Result<Vec<rookie_draft_lottery_pick::Model>>
where
    C: ConnectionTrait,
{
    let pick_models = rookie_draft_lottery_pick::Entity::find()
        .filter(rookie_draft_lottery_pick::Column::RookieDraftLotteryId.eq(rookie_draft_lottery_id))
        .order_by_asc(rookie_draft_lottery_pick::Column::PickNumber)
        .all(db)
        .await?;
    Ok(pick_models)
}

/// Commits the seed for a league season's lottery, or returns the existing row if already committed.
#[instrument(skip(db))]
pub async fn insert_lottery_seed<C>(
    league_id: i64,
    end_of_season_year: i16,
    rng_seed: i64,
    db: &C,
) -> Result<rookie_draft_lottery::Model>
where
    C: ConnectionTrait,
{
    let lottery_to_insert = rookie_draft_lottery::ActiveModel {
        id: ActiveValue::NotSet,
        league_id: ActiveValue::Set(league_id),
        end_of_season_year: ActiveValue::Set(end_of_season_year),
        rng_seed: ActiveValue::Set(rng_seed),
        rng_log: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    // Do-nothing conflict + re-read: a committed seed is never overwritten.
    rookie_draft_lottery::Entity::insert(lottery_to_insert)
        .on_conflict(
            OnConflict::columns([
                rookie_draft_lottery::Column::LeagueId,
                rookie_draft_lottery::Column::EndOfSeasonYear,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    let lottery_model = find_lottery_for_league_season(league_id, end_of_season_year, db)
        .await?
        .ok_or_else(|| {
            color_eyre::eyre::eyre!(
                "lottery row for league ({league_id}) season ({end_of_season_year}) missing right after insert."
            )
        })?;
    Ok(lottery_model)
}

/// Reveals a committed lottery: stores the drawn slots and the per-draw audit log.
#[instrument(skip(drawn_picks, db))]
pub async fn save_lottery_draw<C>(
    rookie_draft_lottery_id: i64,
    drawn_picks: Vec<NewRookieDraftLotteryPick>,
    rng_log: String,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    if drawn_picks.is_empty() {
        return Ok(());
    }

    let picks_to_insert =
        drawn_picks
            .into_iter()
            .map(|drawn_pick| rookie_draft_lottery_pick::ActiveModel {
                id: ActiveValue::NotSet,
                rookie_draft_lottery_id: ActiveValue::Set(rookie_draft_lottery_id),
                pick_number: ActiveValue::Set(drawn_pick.pick_number),
                team_id: ActiveValue::Set(drawn_pick.team_id),
                balls_held: ActiveValue::Set(drawn_pick.balls_held),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            });

    rookie_draft_lottery_pick::Entity::insert_many(picks_to_insert)
        .exec(db)
        .await?;

    rookie_draft_lottery::Entity::update_many()
        .col_expr(
            rookie_draft_lottery::Column::RngLog,
            sea_orm::sea_query::Expr::value(rng_log),
        )
        .filter(rookie_draft_lottery::Column::Id.eq(rookie_draft_lottery_id))
        .exec(db)
        .await?;

    Ok(())
}
