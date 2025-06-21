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
