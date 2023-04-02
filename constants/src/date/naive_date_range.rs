use chrono::NaiveDate;
use color_eyre::Result;

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
