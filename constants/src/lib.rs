use std::collections::HashMap;

use chrono::NaiveDate;
use color_eyre::Result;
use once_cell::sync::Lazy;

pub struct NaiveDateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl NaiveDateRange {
    pub fn from_date_strings(start_date_str: &str, end_date_str: &str) -> Result<Self> {
        Ok(NaiveDateRange {
            start: NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d")?,
            end: NaiveDate::parse_from_str(end_date_str, "%Y-%m-%d")?,
        })
    }
}

pub static NBA_ASB_DATE_RANGES_BY_SEASON_END_YEAR: Lazy<HashMap<i16, NaiveDateRange>> =
    Lazy::new(|| {
        [
            (
                2015,
                NaiveDateRange::from_date_strings("2015-02-13", "2015-02-18").unwrap(),
            ),
            (
                2016,
                NaiveDateRange::from_date_strings("2016-02-12", "2016-02-17").unwrap(),
            ),
            (
                2017,
                NaiveDateRange::from_date_strings("2017-02-17", "2017-02-22").unwrap(),
            ),
            (
                2018,
                NaiveDateRange::from_date_strings("2018-02-16", "2018-02-21").unwrap(),
            ),
            (
                2019,
                NaiveDateRange::from_date_strings("2019-02-15", "2019-02-20").unwrap(),
            ),
            (
                2020,
                NaiveDateRange::from_date_strings("2020-02-14", "2020-02-19").unwrap(),
            ),
            (
                2021,
                NaiveDateRange::from_date_strings("2021-03-05", "2021-03-09").unwrap(),
            ),
            (
                2022,
                NaiveDateRange::from_date_strings("2022-02-18", "2022-02-23").unwrap(),
            ),
            (
                2023,
                NaiveDateRange::from_date_strings("2023-02-17", "2023-02-22").unwrap(),
            ),
        ]
        .into_iter()
        .collect()
    });
