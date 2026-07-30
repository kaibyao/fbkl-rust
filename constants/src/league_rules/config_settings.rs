/// The number of rounds in the rookie draft.
pub static DRAFT_PICK_ROUNDS: i16 = 5;
/// The number of seasons into the future that in which future draft picks can be traded (and therefore generated).
pub static FUTURE_DRAFT_PICK_SEASONS_LIMIT: i16 = 2;
/// The max number of non-(RD|RDI|RFA|UFA) contracts that can be retained by a team at the Keeper Deadline.
pub static KEEPER_CONTRACT_COUNT_LIMIT: usize = 14;
/// The sum of contract values retained by a team for the Keeper Deadline must be at or below this value.
pub static KEEPER_CONTRACT_TOTAL_SALARY_LIMIT: i16 = 100;
/// The maximum number of total contracts a roster can have during the pre-season.
pub static PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT: i16 = 32;
/// The sum of contract values retained by a team for the preseason roster locks (which happen after the keeper deadline and ends with the final pre-season roster lock before the Week 1 FA period) must be at or below this value.
pub static PRE_SEASON_TOTAL_SALARY_LIMIT: i16 = 200;
/// The maximum number of international rookie development contracts a roster can have during the regular or post season.
pub static REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT: i16 = 1;
/// The maximum number of IR slots that can be held on a roster during the regular or post season.
pub static REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT: i16 = 1;
/// The maximum number of (non-international) rookie development contracts a roster can have during the regular or post season.
pub static REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT: i16 = 6;
/// The maximum number of veteran or rookie-scale contracts a roster can have during the regular or post season.
pub static REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT: i16 = 22;
/// The sum of contract values retained by a team for a regular season roster lock must be at or below this value.
pub static REGULAR_SEASON_TOTAL_SALARY_LIMIT: i16 = 210;
/// The sum of contract values retained by a team for a roster lock taking place at or after the auction deadline must be at or below this value.
pub static POST_SEASON_TOTAL_SALARY_LIMIT: i16 = 230;
/// The floor for an in-season free agent auction's opening bid (rules §8.3.3), used unless the
/// player was already owned earlier in the same season.
pub static IN_SEASON_FA_MINIMUM_BID: i16 = 1;
/// How long after its last bid an auction stays open (rules §6.4.4 / §8.3.1).
pub static AUCTION_QUIET_WINDOW_HOURS: i64 = 24;
/// The number of veteran auction players released for bidding each day (rules §6.3.3). The rules set
/// this per season; this is the default until per-season schedule config exists.
pub static VETERAN_AUCTION_PLAYERS_RELEASED_PER_DAY: usize = 15;
/// Length of the RFA-only first week of the veteran auction (rules §6.3.1), after which the rest of
/// the pool starts being released.
pub static VETERAN_AUCTION_RFA_WEEK_DAYS: u64 = 7;
/// UTC offset of the league's wall clock (US Central), which the rules state deadlines in.
// ponytail: fixed standard-time offset, swap for chrono-tz if DST-exact deadlines start mattering.
pub static LEAGUE_TIME_ZONE_UTC_OFFSET_SECONDS: i32 = -6 * 3600;
/// Friday 11:59pm CT: the weekly cutoff for nominating new in-season FA auctions (rules §8.2).
pub static IN_SEASON_FA_OPENING_BID_DEADLINE_HOUR_MINUTE: (u32, u32) = (23, 59);
/// Sunday 8pm CT: the weekly all-bid deadline every open in-season FA auction ends at (rules §8.2).
pub static IN_SEASON_FA_ALL_BID_DEADLINE_HOUR_MINUTE: (u32, u32) = (20, 0);
