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

use chrono::{FixedOffset, TimeDelta, Timelike};
use color_eyre::{Result, eyre::eyre};
use fbkl_constants::league_rules::{
    AUCTION_CRUNCH_EARLIEST_START_HOUR, AUCTION_CRUNCH_QUIET_WINDOW_HOURS,
    AUCTION_CRUNCH_WINDOW_HOURS, AUCTION_QUIET_WINDOW_HOURS, IN_SEASON_FA_EXTENSION_MINUTES,
    IN_SEASON_FA_FIRST_EXTENSION_TRIGGER_MINUTES, IN_SEASON_FA_LATER_EXTENSION_TRIGGER_MINUTES,
    LEAGUE_TIME_ZONE_UTC_OFFSET_SECONDS,
};
use fbkl_entity::{
    auction::AuctionKind,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::{ConnectionTrait, prelude::DateTimeWithTimeZone},
};
use tracing::instrument;

/// The quiet period a bid buys: 24h (rules §6.4.4 / §8.3.1), or 1h once the preseason crunch window
/// has opened. Pass `None` for a mode that has no crunch window.
#[must_use]
pub fn auction_quiet_window(
    now: DateTimeWithTimeZone,
    maybe_crunch_window_start: Option<DateTimeWithTimeZone>,
) -> TimeDelta {
    if maybe_crunch_window_start.is_some_and(|crunch_window_start| now >= crunch_window_start) {
        TimeDelta::hours(AUCTION_CRUNCH_QUIET_WINDOW_HOURS)
    } else {
        TimeDelta::hours(AUCTION_QUIET_WINDOW_HOURS)
    }
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

/// When the preseason crunch window opens: 24h before the hard deadline, moved forward to 8:00am CT
/// if that lands between midnight and 8:00am, since it must not open while owners are asleep.
///
/// Inside it a bid's reprieve drops from 24h to 1h, which is what ends a preseason bidding war
/// before the roster lock — in-season the §8.3.2 chain does that job instead.
pub fn crunch_window_start(hard_deadline: DateTimeWithTimeZone) -> Result<DateTimeWithTimeZone> {
    let league_offset = FixedOffset::east_opt(LEAGUE_TIME_ZONE_UTC_OFFSET_SECONDS)
        .ok_or_else(|| eyre!("Invalid league time zone offset."))?;
    let window_start = hard_deadline
        .checked_sub_signed(TimeDelta::hours(AUCTION_CRUNCH_WINDOW_HOURS))
        .ok_or_else(|| eyre!("crunch window start underflowed from {hard_deadline}"))?
        .with_timezone(&league_offset);
    if window_start.hour() >= AUCTION_CRUNCH_EARLIEST_START_HOUR {
        return Ok(window_start);
    }

    window_start
        .date_naive()
        .and_hms_opt(AUCTION_CRUNCH_EARLIEST_START_HOUR, 0, 0)
        .and_then(|naive| naive.and_local_timezone(league_offset).single())
        .ok_or_else(|| eyre!("Could not move the crunch window start to 8:00am on {window_start}."))
}

/// The clocks an auction's *mode* imposes, as opposed to the ones its own bids set.
#[derive(Clone, Copy, Debug)]
pub struct AuctionModeDeadlines {
    /// The instant past which the auction cannot take bids, whatever its own clocks say: the final
    /// preseason roster lock, or in-season the following week's lock (which is what bounds the
    /// §8.3.2 chain — the rules doc leaves it open-ended). `None` when the season has no lock left.
    pub hard_deadline: Option<DateTimeWithTimeZone>,
    /// When the quiet window shortens to 1h. `None` in-season: bidding is over by Sunday evening,
    /// well before Monday tipoff, so in-season never reaches a crunch window.
    pub crunch_window_start: Option<DateTimeWithTimeZone>,
}

/// Looks up the deadlines an auction of this kind runs against.
#[instrument]
pub async fn find_auction_mode_deadlines<C>(
    kind: AuctionKind,
    league_id: i64,
    end_of_season_year: i16,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<AuctionModeDeadlines>
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
        return Ok(AuctionModeDeadlines {
            hard_deadline: Some(final_roster_lock.date_time),
            crunch_window_start: Some(crunch_window_start(final_roster_lock.date_time)?),
        });
    }

    let maybe_next_roster_lock = deadline_queries::find_next_deadline_for_season_by_datetime(
        league_id,
        end_of_season_year,
        now,
        Some(DeadlineKind::InSeasonRosterLock),
        db,
    )
    .await?;
    Ok(AuctionModeDeadlines {
        hard_deadline: maybe_next_roster_lock.map(|roster_lock| roster_lock.date_time),
        crunch_window_start: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        DateTimeWithTimeZone, TimeDelta, auction_close_at, auction_quiet_window,
        crunch_window_start, rolled_all_bid_deadline,
    };

    fn at(time: &str) -> DateTimeWithTimeZone {
        format!("2024-11-17T{time}:00-06:00").parse().unwrap()
    }

    fn on(date: &str, time: &str) -> DateTimeWithTimeZone {
        format!("{date}T{time}:00-06:00").parse().unwrap()
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
    fn the_crunch_window_opens_a_day_before_the_hard_deadline() {
        assert_eq!(
            crunch_window_start(on("2024-10-18", "19:00")).unwrap(),
            on("2024-10-17", "19:00")
        );
    }

    #[test]
    fn a_crunch_window_that_would_open_overnight_waits_for_8am() {
        // A 3:00am hard deadline puts the window at 3:00am the day before; owners are asleep.
        assert_eq!(
            crunch_window_start(on("2024-10-18", "03:00")).unwrap(),
            on("2024-10-17", "08:00")
        );
        // 8:00am exactly is late enough to stand.
        assert_eq!(
            crunch_window_start(on("2024-10-18", "08:00")).unwrap(),
            on("2024-10-17", "08:00")
        );
    }

    #[test]
    fn the_crunch_window_shortens_the_reprieve_to_an_hour() {
        let crunch_window_start = at("08:00");
        assert_eq!(
            auction_quiet_window(at("07:59"), Some(crunch_window_start)),
            TimeDelta::hours(24)
        );
        assert_eq!(
            auction_quiet_window(at("08:00"), Some(crunch_window_start)),
            TimeDelta::hours(1)
        );
        // In-season has no crunch window, so a bid always buys the full 24h.
        assert_eq!(
            auction_quiet_window(at("08:00"), None),
            TimeDelta::hours(24)
        );
    }

    #[test]
    fn a_bid_inside_the_crunch_window_cannot_push_past_the_hard_deadline() {
        let hard_deadline = at("20:00");
        assert_eq!(
            auction_close_at(
                at("19:30"),
                auction_quiet_window(at("19:30"), Some(at("08:00"))),
                None,
                Some(hard_deadline)
            )
            .unwrap(),
            hard_deadline
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
