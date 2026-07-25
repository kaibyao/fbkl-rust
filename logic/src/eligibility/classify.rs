//! Pure eligibility classifier (spec 10 / rules §3.1.2, §6.2.1, §7.5).
//!
//! Classification is never stored — only the commissioner override is. Everything else is derived
//! from the current NBA-roster fact so a mid-cycle NBA signing (§11.3.1) flips the pool membership
//! on the next read.

use fbkl_entity::{
    league_player,
    player::{self, EligibilityClassification},
};

/// The minimal facts `classify_player` needs, so it stays DB-free and unit-testable.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerEligibilityFacts {
    /// Rules §3.1.2 pivot: has the player ever been on an active NBA roster?
    pub has_been_on_nba_roster: bool,
    /// Commissioner override of the derived classification; wins when set.
    pub eligibility_override: Option<EligibilityClassification>,
    /// A league-created (drafted, not-yet-NBA) player.
    pub is_league_player: bool,
    /// The `is_rdi_eligible` cache — treated as a draft-eligible flag, correctable by validators.
    pub is_flagged_draft_eligible: bool,
}

impl From<&player::Model> for PlayerEligibilityFacts {
    fn from(model: &player::Model) -> Self {
        Self {
            has_been_on_nba_roster: model.has_been_on_nba_roster,
            eligibility_override: model.eligibility_override,
            is_league_player: false,
            is_flagged_draft_eligible: model.is_rdi_eligible,
        }
    }
}

impl From<&league_player::Model> for PlayerEligibilityFacts {
    fn from(model: &league_player::Model) -> Self {
        Self {
            has_been_on_nba_roster: model.has_been_on_nba_roster,
            eligibility_override: model.eligibility_override,
            is_league_player: true,
            is_flagged_draft_eligible: model.is_rdi_eligible,
        }
    }
}

/// Derives which acquisition pool a player belongs to, in the spec's precedence order.
#[must_use]
pub const fn classify_player(facts: PlayerEligibilityFacts) -> EligibilityClassification {
    if let Some(classification) = facts.eligibility_override {
        return classification;
    }
    if facts.has_been_on_nba_roster {
        return EligibilityClassification::VeteranAuctionEligible;
    }
    // §7.5.1 sub-categories aren't distinguishable from current data — see spec 12 for source tags.
    if facts.is_league_player || facts.is_flagged_draft_eligible {
        return EligibilityClassification::RookieDraftEligible;
    }
    EligibilityClassification::Ineligible
}

#[cfg(test)]
mod tests {
    use super::{PlayerEligibilityFacts, classify_player};
    use fbkl_entity::player::EligibilityClassification::{
        Ineligible, RookieDraftEligible, VeteranAuctionEligible,
    };

    #[test]
    fn override_beats_derived_classification() {
        let facts = PlayerEligibilityFacts {
            has_been_on_nba_roster: true,
            eligibility_override: Some(Ineligible),
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts), Ineligible);
    }

    #[test]
    fn nba_roster_history_means_veteran_auction() {
        let facts = PlayerEligibilityFacts {
            has_been_on_nba_roster: true,
            is_league_player: true,
            is_flagged_draft_eligible: true,
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts), VeteranAuctionEligible);
    }

    #[test]
    fn drafted_league_player_is_rookie_draft_eligible() {
        let facts = PlayerEligibilityFacts {
            is_league_player: true,
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts), RookieDraftEligible);
    }

    #[test]
    fn flagged_draft_eligible_real_player_is_rookie_draft_eligible() {
        let facts = PlayerEligibilityFacts {
            is_flagged_draft_eligible: true,
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts), RookieDraftEligible);
    }

    #[test]
    fn never_on_nba_roster_and_unflagged_is_ineligible() {
        assert_eq!(
            classify_player(PlayerEligibilityFacts::default()),
            Ineligible
        );
    }
}
