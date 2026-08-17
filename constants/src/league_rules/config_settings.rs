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
/// How long before a preseason auction's hard deadline the crunch window opens (spec 01 timing rules).
pub static AUCTION_CRUNCH_WINDOW_HOURS: i64 = 24;
/// The quiet period a bid buys once the preseason crunch window has opened (rules §6.4.4).
pub static AUCTION_CRUNCH_QUIET_WINDOW_HOURS: i64 = 1;
/// The crunch window never opens before this hour CT — owners are asleep before it.
pub static AUCTION_CRUNCH_EARLIEST_START_HOUR: u32 = 8;
/// How far a qualifying late bid pushes the in-season all-bid deadline out (rules §8.3.2).
pub static IN_SEASON_FA_EXTENSION_MINUTES: i64 = 30;
/// A bid this close to the week's *original* 8pm all-bid deadline extends it (§8.3.2's "Sunday 7:00 PM-8:00 PM CT").
pub static IN_SEASON_FA_FIRST_EXTENSION_TRIGGER_MINUTES: i64 = 60;
/// Once extended, a bid this close to the current deadline extends it again, until that many quiet minutes pass (§8.3.2).
pub static IN_SEASON_FA_LATER_EXTENSION_TRIGGER_MINUTES: i64 = 30;
/// How long the RFA winning bidder has to raise his bid after the auction closes (rules §15.3.2.1).
pub static RFA_RAISE_WINDOW_HOURS: i64 = 48;
/// How long the RFA winner then has to name the pick he would forfeit (rules §15.2.2).
pub static RFA_PICK_SELECTION_WINDOW_HOURS: i64 = 24;
/// How long the original owner then has to match or decline the RFA bid (rules §15.3.2).
pub static RFA_MATCH_WINDOW_HOURS: i64 = 48;
/// The number of veteran auction players released for bidding each day (rules §6.3.3). The rules set
/// this per season; this is the default until per-season schedule config exists.
pub static VETERAN_AUCTION_PLAYERS_RELEASED_PER_DAY: usize = 15;
/// Length of the RFA-only first week of the veteran auction (rules §6.3.1), after which the rest of
/// the pool starts being released.
pub static VETERAN_AUCTION_RFA_WEEK_DAYS: u64 = 7;
/// Friday 11:59pm CT: the weekly cutoff for nominating new in-season FA auctions (rules §8.2).
pub static IN_SEASON_FA_OPENING_BID_DEADLINE_HOUR_MINUTE: (u32, u32) = (23, 59);
/// Sunday 8pm CT: the weekly all-bid deadline every open in-season FA auction ends at (rules §8.2).
pub static IN_SEASON_FA_ALL_BID_DEADLINE_HOUR_MINUTE: (u32, u32) = (20, 0);
/// Fixed Rookie-Development contract salary for a rookie drafted in the given round (rules §7.4.1).
/// Index 0 = round 1. Rounds 1–5 → $4/$3/$2/$1/$1.
pub static ROOKIE_DRAFT_ROUND_SALARIES: [i16; 5] = [4, 3, 2, 1, 1];
/// Lottery balls per non-playoff seed, worst → best (rules §7.2.4). 6 non-playoff seeds.
pub static ROOKIE_DRAFT_LOTTERY_BALLS: [u32; 6] = [6, 5, 4, 3, 2, 1];

/// Rookie-Development salary for a 1-based rookie draft round (rules §7.4.1).
///
/// # Panics
/// Panics if `round` is outside 1..=[`DRAFT_PICK_ROUNDS`]; a draft round out of range is a
/// programmer error, not a recoverable condition.
#[must_use]
pub fn rookie_draft_salary_for_round(round: i16) -> i16 {
    round
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| ROOKIE_DRAFT_ROUND_SALARIES.get(index).copied())
        .unwrap_or_else(|| {
            panic!("Rookie draft round ({round}) is outside 1..={DRAFT_PICK_ROUNDS}.")
        })
}

#[cfg(test)]
mod tests {
    use super::{DRAFT_PICK_ROUNDS, ROOKIE_DRAFT_ROUND_SALARIES, rookie_draft_salary_for_round};

    #[test]
    fn salary_for_each_valid_round() {
        assert_eq!(
            (1..=DRAFT_PICK_ROUNDS)
                .map(rookie_draft_salary_for_round)
                .collect::<Vec<_>>(),
            ROOKIE_DRAFT_ROUND_SALARIES.to_vec()
        );
    }

    #[test]
    #[should_panic(expected = "Rookie draft round (0) is outside 1..=5.")]
    fn round_zero_panics() {
        let _ = rookie_draft_salary_for_round(0);
    }

    #[test]
    #[should_panic(expected = "Rookie draft round (6) is outside 1..=5.")]
    fn round_past_last_round_panics() {
        let _ = rookie_draft_salary_for_round(6);
    }

    #[test]
    #[should_panic(expected = "Rookie draft round (-1) is outside 1..=5.")]
    fn negative_round_panics() {
        let _ = rookie_draft_salary_for_round(-1);
    }
}
