/// The bid tiers that decide which Rookie-Draft pick an owner forfeits when he signs another
/// team's RFA and that team declines to match (rules §15.2.1).
///
/// Each entry is `(max_bid_inclusive, round)`, ordered by ascending bid. The round is "or better",
/// so it is the *highest* round number the winner may forfeit; an earlier (lower-numbered) round
/// also satisfies the tier. Bids above the last cap owe [`RFA_COMPENSATION_BEST_ROUND`].
pub static RFA_COMPENSATION_TIERS: [(i16, i16); 4] = [(11, 5), (18, 4), (27, 3), (41, 2)];

/// The round owed for a bid above the last cap in [`RFA_COMPENSATION_TIERS`] (rules §15.2.1: ≥ $42).
pub static RFA_COMPENSATION_BEST_ROUND: i16 = 1;

/// The highest Rookie-Draft round number a winning bidder may forfeit as RFA compensation for
/// `final_bid` (rules §15.2.1). A pick from any earlier round is also acceptable.
#[must_use]
pub fn compensation_round_for_bid(final_bid: i16) -> i16 {
    RFA_COMPENSATION_TIERS
        .iter()
        .find(|(max_bid_inclusive, _)| final_bid <= *max_bid_inclusive)
        .map_or(RFA_COMPENSATION_BEST_ROUND, |(_, round)| *round)
}

#[cfg(test)]
mod tests {
    use super::compensation_round_for_bid;

    #[test]
    fn round_for_each_tier_boundary() {
        assert_eq!(
            [1, 11, 12, 18, 19, 27, 28, 41, 42, 500].map(compensation_round_for_bid),
            [5, 5, 4, 4, 3, 3, 2, 2, 1, 1]
        );
    }
}
