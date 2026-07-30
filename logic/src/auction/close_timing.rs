//! When an auction stops taking bids (rules §6.4.4 / §8.3.1-.2, spec 01 "Timing rules").
//!
//! Every auction, in every mode, closes by one rule:
//!
//! ```text
//! close_at = min(last_bid + quiet_window, all_bid_deadline)   clamped to the hard deadline
//! ```
//!
//! The hard deadline is absolute: an auction still taking bids is force-closed there and the last
//! bidder wins. `close_at` is stored on the auction rather than derived per tick, so the close tick
//! stays one indexed `close_at <= now` scan and the bid path has a single value to compare against.
//! Every site that writes `close_at` computes it here, so no write site can forget a clock.
//!
//! An unbid *veteran* auction is the one auction without a bid to measure from: its clock is the
//! §6.3.4 tier ladder, so the daily slide pushes `close_at` out another day and the day the slide
//! finds no lower tier, the lapsed `close_at` expires it.

use std::fmt::Debug;

use chrono::TimeDelta;
use color_eyre::{Result, eyre::eyre};
use fbkl_constants::league_rules::AUCTION_QUIET_WINDOW_HOURS;
use fbkl_entity::{
    auction::AuctionKind,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::{ConnectionTrait, prelude::DateTimeWithTimeZone},
};
use tracing::instrument;

/// The quiet period a bid buys (rules §6.4.4 / §8.3.1).
#[must_use]
pub fn auction_quiet_window() -> TimeDelta {
    TimeDelta::hours(AUCTION_QUIET_WINDOW_HOURS)
}

/// When an auction stops taking bids: the last bid's quiet window, cut short by whichever of the
/// all-bid deadline (§8.2.2) and the hard deadline arrives first. Both are absolute.
pub fn auction_close_at(
    last_bid_at: DateTimeWithTimeZone,
    quiet_window: TimeDelta,
    maybe_all_bid_deadline: Option<DateTimeWithTimeZone>,
    maybe_hard_deadline: Option<DateTimeWithTimeZone>,
) -> Result<DateTimeWithTimeZone> {
    let quiet_window_end = last_bid_at
        .checked_add_signed(quiet_window)
        .ok_or_else(|| eyre!("auction quiet window overflowed from {last_bid_at}"))?;

    Ok([maybe_all_bid_deadline, maybe_hard_deadline]
        .into_iter()
        .flatten()
        .fold(quiet_window_end, Ord::min))
}

/// The instant past which an auction of this kind cannot take bids, whatever its own clocks say.
///
/// Preseason auctions cannot outlive the final preseason roster lock; in-season FA cannot outlive
/// the following week's roster lock, which is what bounds the §8.3.2 extension chain (the rules doc
/// leaves it open-ended). `None` only when the season has no in-season lock left to clamp against.
#[instrument]
pub async fn find_auction_hard_deadline<C>(
    kind: AuctionKind,
    league_id: i64,
    end_of_season_year: i16,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<Option<DateTimeWithTimeZone>>
where
    C: ConnectionTrait + Debug,
{
    if kind.is_preseason() {
        let final_roster_lock = deadline_queries::find_deadline_for_season_by_type(
            league_id,
            end_of_season_year,
            DeadlineKind::PreseasonFinalRosterLock,
            db,
        )
        .await?;
        return Ok(Some(final_roster_lock.date_time));
    }

    let maybe_next_roster_lock = deadline_queries::find_next_deadline_for_season_by_datetime(
        league_id,
        end_of_season_year,
        now,
        Some(DeadlineKind::InSeasonRosterLock),
        db,
    )
    .await?;
    Ok(maybe_next_roster_lock.map(|roster_lock| roster_lock.date_time))
}

#[cfg(test)]
mod tests {
    use super::{DateTimeWithTimeZone, TimeDelta, auction_close_at};

    fn at(time: &str) -> DateTimeWithTimeZone {
        format!("2024-11-17T{time}:00-06:00").parse().unwrap()
    }

    fn quiet_window() -> TimeDelta {
        TimeDelta::hours(24)
    }

    #[test]
    fn a_bid_buys_its_full_quiet_window_when_nothing_else_binds() {
        assert_eq!(
            auction_close_at(at("12:00"), quiet_window(), None, None).unwrap(),
            "2024-11-18T12:00:00-06:00"
                .parse::<DateTimeWithTimeZone>()
                .unwrap()
        );
    }

    #[test]
    fn the_all_bid_deadline_cuts_the_quiet_window_short() {
        assert_eq!(
            auction_close_at(at("12:00"), quiet_window(), Some(at("20:00")), None).unwrap(),
            at("20:00")
        );
    }

    #[test]
    fn the_hard_deadline_cuts_the_quiet_window_short() {
        assert_eq!(
            auction_close_at(at("12:00"), quiet_window(), None, Some(at("18:00"))).unwrap(),
            at("18:00")
        );
    }

    #[test]
    fn the_hard_deadline_also_beats_a_later_all_bid_deadline() {
        assert_eq!(
            auction_close_at(
                at("12:00"),
                quiet_window(),
                Some(at("20:00")),
                Some(at("18:00"))
            )
            .unwrap(),
            at("18:00")
        );
    }

    #[test]
    fn a_short_quiet_window_still_wins_when_it_ends_first() {
        assert_eq!(
            auction_close_at(
                at("12:00"),
                TimeDelta::hours(1),
                Some(at("20:00")),
                Some(at("18:00"))
            )
            .unwrap(),
            at("13:00")
        );
    }
}
