pub mod date;
pub mod league_rules;

/// The pseudo-team standing in for free agents (players not on any real NBA team).
pub struct FreeAgencyTeam {
    pub city: &'static str,
    pub name: &'static str,
    pub abbr: &'static str,
    pub nba_id: i32,
    pub espn_id: i16,
}

pub static FREE_AGENCY_TEAM: FreeAgencyTeam = FreeAgencyTeam {
    city: "Free",
    name: "Agency",
    abbr: "FA",
    nba_id: 0,
    espn_id: 0,
};
