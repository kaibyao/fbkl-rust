//! Pure eligibility classifier (spec 10 / rules §3.1.2, §6.2.1, §7.5).
//!
//! Classification is never stored — only the commissioner override is — and it is always relative to
//! a **season**, never to "now". The same player is rookie-draft-eligible the season before his NBA
//! debut and a veteran the season after, so every caller states which season it is asking about:
//! pool builders pass the pool's season, contract guards pass the contract's, and historical replay
//! therefore gets the answer that was true at the time.
//!
//! Pool membership pivots on games played (§3.1.2); the broader "was on an NBA roster" (§3.1.3) only
//! gates RDI — see `super::rdi`.

use fbkl_entity::{
    league_player,
    player::{self, EligibilityClassification},
};

/// The minimal facts `classify_player` needs, so it stays DB-free and unit-testable.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerEligibilityFacts {
    /// Whether the player ever appeared in an in-season NBA game, over his whole career.
    pub has_played_nba_game: bool,
    /// The season the player first appeared in NBA data, if he ever did. Turns the two career facts
    /// above and below into as-of-a-season answers.
    pub nba_first_season_end_of_season_year: Option<i16>,
    /// Commissioner override of the derived classification; wins when set.
    pub eligibility_override: Option<EligibilityClassification>,
    /// A league-created (drafted, not-yet-NBA) player.
    pub is_league_player: bool,
    /// The `is_rdi_eligible` cache — treated as a draft-eligible flag, correctable by validators.
    pub is_flagged_draft_eligible: bool,
}

impl PlayerEligibilityFacts {
    /// Rules §3.1.3 entering a season: had the player any NBA entry in an *earlier* season?
    ///
    /// Strictly earlier, not same-or-earlier, because the stored season is only season-granular
    /// while the rules turn on where in the season the player arrived. §11.3.5 gives an RDI who
    /// lands on an NBA roster mid-season until the *next* legalization to move, and a mid-season
    /// signing or a draft-and-stash both record the season they arrived in — so `<=` would reject
    /// exactly the legalization the rule protects.
    #[must_use]
    pub const fn was_on_nba_roster_before(&self, end_of_season_year: i16) -> bool {
        match self.nba_first_season_end_of_season_year {
            Some(first_season) => first_season < end_of_season_year,
            None => false,
        }
    }

    /// Rules §3.1.2 entering a season: had the player appeared in a game in an earlier season?
    ///
    /// The stored `has_played_nba_game` is a career fact with no date of its own, so his first NBA
    /// season stands in for his debut. A player rostered seasons before he first appeared reads as
    /// having debuted too early; the commissioner override settles those.
    #[must_use]
    pub const fn had_played_nba_game_before(&self, end_of_season_year: i16) -> bool {
        self.has_played_nba_game && self.was_on_nba_roster_before(end_of_season_year)
    }
}

impl From<&player::Model> for PlayerEligibilityFacts {
    fn from(model: &player::Model) -> Self {
        Self {
            has_played_nba_game: model.has_played_nba_game,
            nba_first_season_end_of_season_year: model.nba_first_season_end_of_season_year,
            eligibility_override: model.eligibility_override,
            is_league_player: false,
            is_flagged_draft_eligible: model.is_rdi_eligible,
        }
    }
}

impl From<&league_player::Model> for PlayerEligibilityFacts {
    fn from(model: &league_player::Model) -> Self {
        Self {
            has_played_nba_game: model.has_played_nba_game,
            nba_first_season_end_of_season_year: model.nba_first_season_end_of_season_year,
            eligibility_override: model.eligibility_override,
            is_league_player: true,
            is_flagged_draft_eligible: model.is_rdi_eligible,
        }
    }
}

/// Derives which acquisition pool a player belongs to entering `end_of_season_year`.
///
/// Follows the spec's precedence order. A player who debuts during that season is still
/// draft-eligible for it and becomes a veteran for the next one, matching when the auction and
/// draft actually run.
#[must_use]
pub const fn classify_player(
    facts: PlayerEligibilityFacts,
    end_of_season_year: i16,
) -> EligibilityClassification {
    if let Some(classification) = facts.eligibility_override {
        return classification;
    }
    if facts.had_played_nba_game_before(end_of_season_year) {
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
            has_played_nba_game: true,
            nba_first_season_end_of_season_year: Some(2020),
            eligibility_override: Some(Ineligible),
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts, 2025), Ineligible);
    }

    #[test]
    fn nba_game_history_means_veteran_auction() {
        let facts = PlayerEligibilityFacts {
            has_played_nba_game: true,
            nba_first_season_end_of_season_year: Some(2020),
            is_league_player: true,
            is_flagged_draft_eligible: true,
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts, 2025), VeteranAuctionEligible);
    }

    /// §3.1.2 — a rostered player who never appeared in a game stays in the rookie draft pool.
    #[test]
    fn nba_roster_without_a_game_is_still_rookie_draft_eligible() {
        let facts = PlayerEligibilityFacts {
            nba_first_season_end_of_season_year: Some(2020),
            is_flagged_draft_eligible: true,
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts, 2025), RookieDraftEligible);
    }

    /// Leandro Bolmaro: drafted 2020, overseas for 2021, first NBA season 2022. One set of stored
    /// facts answers all three seasons differently, which is the whole point of the season argument.
    #[test]
    fn classification_is_relative_to_the_season_asked_about() {
        let facts = PlayerEligibilityFacts {
            has_played_nba_game: true,
            nba_first_season_end_of_season_year: Some(2022),
            is_flagged_draft_eligible: true,
            ..PlayerEligibilityFacts::default()
        };

        assert_eq!(classify_player(facts, 2021), RookieDraftEligible);
        assert!(!facts.was_on_nba_roster_before(2021));
        // §11.3.5 — arriving during 2022 does not retroactively bar 2022's RD→RDI move.
        assert_eq!(classify_player(facts, 2022), RookieDraftEligible);
        assert!(!facts.was_on_nba_roster_before(2022));
        assert_eq!(classify_player(facts, 2023), VeteranAuctionEligible);
        assert!(facts.was_on_nba_roster_before(2023));
    }

    #[test]
    fn drafted_league_player_is_rookie_draft_eligible() {
        let facts = PlayerEligibilityFacts {
            is_league_player: true,
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts, 2025), RookieDraftEligible);
    }

    #[test]
    fn flagged_draft_eligible_real_player_is_rookie_draft_eligible() {
        let facts = PlayerEligibilityFacts {
            is_flagged_draft_eligible: true,
            ..PlayerEligibilityFacts::default()
        };
        assert_eq!(classify_player(facts, 2025), RookieDraftEligible);
    }

    #[test]
    fn never_played_and_unflagged_is_ineligible() {
        assert_eq!(
            classify_player(PlayerEligibilityFacts::default(), 2025),
            Ineligible
        );
    }
}
