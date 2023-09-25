use chrono::FixedOffset;
use once_cell::sync::Lazy;

pub static CENTRAL_TIMEZONE_OFFSET_HOURS: i32 = 6;
pub static CENTRAL_TIMEZONE_OFFSET: Lazy<FixedOffset> =
    Lazy::new(|| FixedOffset::west_opt(3600 * CENTRAL_TIMEZONE_OFFSET_HOURS).unwrap());
