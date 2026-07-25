//! Reads/writes for the veteran auction release schedule and its minimum-bid tiers (rules §6.3).

use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, prelude::Date,
};
use tracing::instrument;

use crate::{auction_schedule, min_bid_tier_config};

/// One pooled player's scheduled release, as built by veteran pool assembly.
#[derive(Clone, Copy, Debug)]
pub struct NewAuctionScheduleRow {
    pub player_id: i64,
    pub scheduled_release_date: Date,
    pub nomination_rank: Option<i16>,
    pub min_bid_tier: i16,
    pub is_rfa_week: bool,
}

#[instrument(skip(rows))]
pub async fn insert_auction_schedule_rows<C>(
    league_id: i64,
    end_of_season_year: i16,
    rows: Vec<NewAuctionScheduleRow>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    if rows.is_empty() {
        return Ok(());
    }

    let models_to_insert = rows.into_iter().map(|row| auction_schedule::ActiveModel {
        id: ActiveValue::NotSet,
        league_id: ActiveValue::Set(league_id),
        end_of_season_year: ActiveValue::Set(end_of_season_year),
        player_id: ActiveValue::Set(row.player_id),
        scheduled_release_date: ActiveValue::Set(row.scheduled_release_date),
        nomination_rank: ActiveValue::Set(row.nomination_rank),
        min_bid_tier: ActiveValue::Set(row.min_bid_tier),
        is_rfa_week: ActiveValue::Set(row.is_rfa_week),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    });
    auction_schedule::Entity::insert_many(models_to_insert)
        .exec(db)
        .await?;

    Ok(())
}

/// Schedule rows whose release date has arrived, ranked players first (rules §6.3.3).
#[instrument]
pub async fn find_auction_schedule_rows_due_for_release<C>(
    league_id: i64,
    end_of_season_year: i16,
    today: Date,
    db: &C,
) -> Result<Vec<auction_schedule::Model>>
where
    C: ConnectionTrait + Debug,
{
    let schedule_models = auction_schedule::Entity::find()
        .filter(auction_schedule::Column::LeagueId.eq(league_id))
        .filter(auction_schedule::Column::EndOfSeasonYear.eq(end_of_season_year))
        .filter(auction_schedule::Column::ScheduledReleaseDate.lte(today))
        .order_by_asc(auction_schedule::Column::ScheduledReleaseDate)
        .order_by_asc(auction_schedule::Column::NominationRank)
        .all(db)
        .await?;
    Ok(schedule_models)
}

/// The season's tiers, top tier first.
#[instrument]
pub async fn find_min_bid_tiers<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<min_bid_tier_config::Model>>
where
    C: ConnectionTrait + Debug,
{
    let tier_models = min_bid_tier_config::Entity::find()
        .filter(min_bid_tier_config::Column::LeagueId.eq(league_id))
        .filter(min_bid_tier_config::Column::EndOfSeasonYear.eq(end_of_season_year))
        .order_by_asc(min_bid_tier_config::Column::TierIndex)
        .all(db)
        .await?;
    Ok(tier_models)
}

#[instrument]
pub async fn find_min_bid_tier_by_index<C>(
    league_id: i64,
    end_of_season_year: i16,
    tier_index: i16,
    db: &C,
) -> Result<Option<min_bid_tier_config::Model>>
where
    C: ConnectionTrait + Debug,
{
    let maybe_tier_model = min_bid_tier_config::Entity::find()
        .filter(min_bid_tier_config::Column::LeagueId.eq(league_id))
        .filter(min_bid_tier_config::Column::EndOfSeasonYear.eq(end_of_season_year))
        .filter(min_bid_tier_config::Column::TierIndex.eq(tier_index))
        .one(db)
        .await?;
    Ok(maybe_tier_model)
}

/// The highest configured tier strictly below `current_min_bid_amount` — the single step an unbid
/// auction slides down (rules §6.3.4). `None` at the bottom tier, which never slides further.
#[instrument]
pub async fn find_next_lower_min_bid_tier<C>(
    league_id: i64,
    end_of_season_year: i16,
    current_min_bid_amount: i16,
    db: &C,
) -> Result<Option<min_bid_tier_config::Model>>
where
    C: ConnectionTrait + Debug,
{
    let maybe_tier_model = min_bid_tier_config::Entity::find()
        .filter(min_bid_tier_config::Column::LeagueId.eq(league_id))
        .filter(min_bid_tier_config::Column::EndOfSeasonYear.eq(end_of_season_year))
        .filter(min_bid_tier_config::Column::MinBidAmount.lt(current_min_bid_amount))
        .order_by_desc(min_bid_tier_config::Column::MinBidAmount)
        .one(db)
        .await?;
    Ok(maybe_tier_model)
}
