use chrono::FixedOffset;

pub static CENTRAL_TIMEZONE_OFFSET_HOURS: i32 = 6;
pub static CENTRAL_TIMEZONE_OFFSET: std::sync::LazyLock<FixedOffset> =
    std::sync::LazyLock::new(|| {
        FixedOffset::west_opt(3600 * CENTRAL_TIMEZONE_OFFSET_HOURS).unwrap()
    });
