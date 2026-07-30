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
use fbkl_constants::league_rules::{
    AUCTION_QUIET_WINDOW_HOURS, IN_SEASON_FA_EXTENSION_MINUTES,
    IN_SEASON_FA_FIRST_EXTENSION_TRIGGER_MINUTES, IN_SEASON_FA_LATER_EXTENSION_TRIGGER_MINUTES,
};
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

/// The §8.3.2 roll a late in-season bid earns: 30 minutes onto the all-bid deadline.
///
/// So the auction only ends once 30 quiet minutes pass. `None` when the bid is too early to move the
/// deadline, or when the hard deadline already caps it — an extension chain may not run past the
/// coming roster lock.
///
/// The trigger has **two widths**: 60 minutes while the deadline is still the week's original 8pm
/// one, 30 minutes once it has been extended. A flat 30 would let §8.5's 7:15pm bid close the
/// auction at 8:00pm instead of extending it to 8:30pm.
#[must_use]
pub fn rolled_all_bid_deadline(
    now: DateTimeWithTimeZone,
    all_bid_deadline: DateTimeWithTimeZone,
    original_all_bid_deadline: DateTimeWithTimeZone,
    maybe_hard_deadline: Option<DateTimeWithTimeZone>,
) -> Option<DateTimeWithTimeZone> {
    let trigger_width = if all_bid_deadline == original_all_bid_deadline {
        TimeDelta::minutes(IN_SEASON_FA_FIRST_EXTENSION_TRIGGER_MINUTES)
    } else {
        TimeDelta::minutes(IN_SEASON_FA_LATER_EXTENSION_TRIGGER_MINUTES)
    };
    if now >= all_bid_deadline || all_bid_deadline - now > trigger_width {
        return None;
    }

    let rolled =
        all_bid_deadline.checked_add_signed(TimeDelta::minutes(IN_SEASON_FA_EXTENSION_MINUTES))?;
    let clamped = maybe_hard_deadline.map_or(rolled, |hard_deadline| rolled.min(hard_deadline));
    (clamped > all_bid_deadline).then_some(clamped)
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
    use super::{DateTimeWithTimeZone, TimeDelta, auction_close_at, rolled_all_bid_deadline};

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

    /// Rules §8.5's worked example, from the real 8:00pm deadline (an 8:30pm one passes either way).
    #[test]
    fn the_extension_chain_reproduces_the_worked_example() {
        let original = at("20:00");

        // $5 at 7:15pm — 45 min out, inside the first 60-min width: a flat 30 fails right here.
        let after_first_bid = rolled_all_bid_deadline(at("19:15"), original, original, None);
        assert_eq!(after_first_bid, Some(at("20:30")));

        // $6 at 7:42pm — 48 min from the extended deadline, and the width is now 30 min: no change.
        let extended = after_first_bid.unwrap();
        assert_eq!(
            rolled_all_bid_deadline(at("19:42"), extended, original, None),
            None
        );

        // $7 at 8:13pm — 17 min out, inside the 30-minute width, so 8:30pm becomes 9:00pm.
        let after_last_bid = rolled_all_bid_deadline(at("20:13"), extended, original, None);
        assert_eq!(after_last_bid, Some(at("21:00")));

        // A bid in the 30 quiet minutes after is too early to roll again: the 8:13pm bidder wins.
        assert_eq!(
            rolled_all_bid_deadline(at("20:25"), after_last_bid.unwrap(), original, None),
            None
        );
    }

    #[test]
    fn a_bid_before_the_first_trigger_width_leaves_the_deadline_alone() {
        let original = at("20:00");
        assert_eq!(
            rolled_all_bid_deadline(at("18:59"), original, original, None),
            None
        );
    }

    #[test]
    fn an_extension_chain_cannot_roll_past_the_hard_deadline() {
        let original = at("20:00");
        let roster_lock = at("20:15");

        // The +30min roll would reach 8:30pm; the lock caps it at 8:15pm.
        assert_eq!(
            rolled_all_bid_deadline(at("19:30"), original, original, Some(roster_lock)),
            Some(roster_lock)
        );
        // Already at the lock: a later bid earns nothing, so the chain stops there.
        assert_eq!(
            rolled_all_bid_deadline(at("20:10"), roster_lock, original, Some(roster_lock)),
            None
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
