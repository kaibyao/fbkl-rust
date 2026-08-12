//! The league's wall clock.
//!
//! The rules state every deadline in US Central time ("Sunday 8pm CT", "Friday 11:59pm CT"), and
//! America/Chicago observes daylight saving — UTC-5 from roughly March to November, UTC-6 the rest
//! of the year. That DST half covers the start of every NBA season and the March-June tail, so a
//! fixed offset puts those deadlines an hour off for most of the season. Build every wall-clock
//! deadline through [`league_wall_clock`] instead.

use chrono::{DateTime, FixedOffset, NaiveDateTime};
use chrono_tz::{America::Chicago, Tz};
use color_eyre::{Result, eyre::eyre};

/// The zone the rules state deadlines in.
pub static LEAGUE_TIME_ZONE: Tz = Chicago;

/// The instant a Central wall-clock time lands on, DST included.
///
/// # Errors
/// Errors when the wall clock never happens — the hour spring-forward skips. A repeated wall clock
/// (the hour fall-back replays) resolves to the earlier, still-CDT instant.
pub fn league_wall_clock(local: NaiveDateTime) -> Result<DateTime<FixedOffset>> {
    local
        .and_local_timezone(LEAGUE_TIME_ZONE)
        .earliest()
        .map(|zoned| zoned.fixed_offset())
        .ok_or_else(|| eyre!("{local} does not exist in {LEAGUE_TIME_ZONE}: DST skips that hour."))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::league_wall_clock;

    fn wall_clock(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Option<String> {
        let naive = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap();
        league_wall_clock(naive)
            .ok()
            .map(|instant| instant.to_rfc3339())
    }

    #[test]
    fn central_wall_clocks_follow_daylight_saving() {
        assert_eq!(
            wall_clock(2026, 3, 7, 20, 0).unwrap(),
            "2026-03-07T20:00:00-06:00"
        );
        assert_eq!(
            wall_clock(2026, 3, 8, 20, 0).unwrap(),
            "2026-03-08T20:00:00-05:00"
        );
        assert_eq!(
            wall_clock(2026, 10, 31, 20, 0).unwrap(),
            "2026-10-31T20:00:00-05:00"
        );
        assert_eq!(
            wall_clock(2026, 11, 1, 20, 0).unwrap(),
            "2026-11-01T20:00:00-06:00"
        );
    }

    #[test]
    fn the_skipped_spring_forward_hour_has_no_instant() {
        assert_eq!(wall_clock(2026, 3, 8, 2, 30), None);
    }

    #[test]
    fn a_repeated_fall_back_hour_takes_the_earlier_instant() {
        assert_eq!(
            wall_clock(2026, 11, 1, 1, 30).unwrap(),
            "2026-11-01T01:30:00-05:00"
        );
    }
}
