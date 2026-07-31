//! Reads/writes for the veteran auction release schedule and its per-season inputs (rules §6.3).
//!
//! The two commissioner inputs — the ordered minimum-bid tiers and the ranked nomination list
//! (§6.3.6) — are entered before the auction starts and replaced wholesale on re-entry, so
//! re-running entry for a season is idempotent.

use std::fmt::Debug;

use color_eyre::{Result, eyre::bail};
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait, prelude::Date,
};
use tracing::instrument;

use crate::{auction_schedule, min_bid_tier_config, veteran_auction_ranking};

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

/// Schedule rows across all leagues whose release date has arrived, ranked players first (§6.3.3).
///
/// Rows stay due once their date passes; opening a row whose auction already exists — open or
/// settled — is a no-op, so there is no consumed state to filter on here.
#[instrument]
pub async fn find_auction_schedule_rows_due_for_release<C>(
    today: Date,
    db: &C,
) -> Result<Vec<auction_schedule::Model>>
where
    C: ConnectionTrait + Debug,
{
    let schedule_models = auction_schedule::Entity::find()
        .filter(auction_schedule::Column::ScheduledReleaseDate.lte(today))
        .order_by_asc(auction_schedule::Column::ScheduledReleaseDate)
        .order_by_asc(auction_schedule::Column::NominationRank)
        .all(db)
        .await?;
    Ok(schedule_models)
}

/// Rejects a tier list the slide rule cannot walk down (rules §6.3.4-.5).
///
/// The slide steps an unbid auction to the next-lower configured value, so a repeated or ascending
/// value would either stall the ladder or run it backwards. Shared with the entry path so the
/// commissioner sees the rejection as a bad request rather than a write failure.
pub fn validate_min_bid_tiers(min_bid_amounts: &[i16]) -> Result<()> {
    let Some(&bottom_tier_amount) = min_bid_amounts.last() else {
        bail!("A season needs at least one minimum bid tier.");
    };
    if bottom_tier_amount < 1 {
        bail!("Minimum bid tiers must be positive, got {bottom_tier_amount}.");
    }
    if min_bid_amounts.windows(2).any(|pair| pair[0] <= pair[1]) {
        bail!("Minimum bid tiers must strictly descend, got {min_bid_amounts:?}.");
    }
    Ok(())
}

/// Replaces the season's minimum-bid tiers with `min_bid_amounts`, top tier first (rules §6.3.6).
#[instrument]
pub async fn set_min_bid_tiers<C>(
    league_id: i64,
    end_of_season_year: i16,
    min_bid_amounts: &[i16],
    db: &C,
) -> Result<Vec<min_bid_tier_config::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    validate_min_bid_tiers(min_bid_amounts)?;

    let db_txn = db.begin().await?;
    min_bid_tier_config::Entity::delete_many()
        .filter(min_bid_tier_config::Column::LeagueId.eq(league_id))
        .filter(min_bid_tier_config::Column::EndOfSeasonYear.eq(end_of_season_year))
        .exec(&db_txn)
        .await?;
    let models_to_insert = min_bid_amounts
        .iter()
        .enumerate()
        .map(|(tier_index, min_bid_amount)| {
            Ok(min_bid_tier_config::ActiveModel {
                id: ActiveValue::NotSet,
                league_id: ActiveValue::Set(league_id),
                end_of_season_year: ActiveValue::Set(end_of_season_year),
                tier_index: ActiveValue::Set(i16::try_from(tier_index)?),
                min_bid_amount: ActiveValue::Set(*min_bid_amount),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    min_bid_tier_config::Entity::insert_many(models_to_insert)
        .exec(&db_txn)
        .await?;
    db_txn.commit().await?;

    find_min_bid_tiers(league_id, end_of_season_year, db).await
}

/// Replaces the season's ranked nomination list, best player first (rules §6.3.2, §6.3.6).
#[instrument]
pub async fn set_veteran_auction_ranking<C>(
    league_id: i64,
    end_of_season_year: i16,
    ranked_player_ids: &[i64],
    db: &C,
) -> Result<Vec<i64>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    if ranked_player_ids.is_empty() {
        bail!("A season's ranked veteran auction list cannot be empty.");
    }

    let db_txn = db.begin().await?;
    veteran_auction_ranking::Entity::delete_many()
        .filter(veteran_auction_ranking::Column::LeagueId.eq(league_id))
        .filter(veteran_auction_ranking::Column::EndOfSeasonYear.eq(end_of_season_year))
        .exec(&db_txn)
        .await?;
    let models_to_insert = ranked_player_ids
        .iter()
        .enumerate()
        .map(|(position, player_id)| {
            Ok(veteran_auction_ranking::ActiveModel {
                id: ActiveValue::NotSet,
                league_id: ActiveValue::Set(league_id),
                end_of_season_year: ActiveValue::Set(end_of_season_year),
                player_id: ActiveValue::Set(*player_id),
                nomination_rank: ActiveValue::Set(i16::try_from(position + 1)?),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    veteran_auction_ranking::Entity::insert_many(models_to_insert)
        .exec(&db_txn)
        .await?;
    db_txn.commit().await?;

    find_veteran_auction_ranked_player_ids(league_id, end_of_season_year, db).await
}

/// The season's ranked nomination list in rank order, empty when the commissioner has not set one.
#[instrument]
pub async fn find_veteran_auction_ranked_player_ids<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<i64>>
where
    C: ConnectionTrait + Debug,
{
    let ranked_player_ids = veteran_auction_ranking::Entity::find()
        .filter(veteran_auction_ranking::Column::LeagueId.eq(league_id))
        .filter(veteran_auction_ranking::Column::EndOfSeasonYear.eq(end_of_season_year))
        .order_by_asc(veteran_auction_ranking::Column::NominationRank)
        .select_only()
        .column(veteran_auction_ranking::Column::PlayerId)
        .into_tuple::<i64>()
        .all(db)
        .await?;
    Ok(ranked_player_ids)
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
