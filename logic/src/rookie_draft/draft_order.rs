//! Rookie draft order computation (§7.2.1).
//!
//! The order is derived from the frozen `league_team_season_standing` inputs plus the lottery
//! result: round 1 is the six drawn lottery slots followed by the playoff teams worst-finish first
//! (champion picks last), and rounds 2–5 repeat one identical order — non-playoff teams in reverse
//! mid-season rank, then playoff teams best-finish first. This is not a snake draft.
//!
//! Each ordered slot belongs to a team's *natural* pick (`original_owner_team_id`) but is made by
//! whoever holds it now (`current_owner_team_id`), so traded picks land with the acquirer.

use std::{cmp::Reverse, collections::HashMap};

use color_eyre::{
    Result,
    eyre::{bail, ensure, eyre},
};
use fbkl_constants::league_rules::DRAFT_PICK_ROUNDS;
use fbkl_entity::{draft_pick, league_team_season_standing};

/// One slot of the ordered draft slate, in overall pick order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftSlot {
    pub round: i16,
    pub draft_pick_id: i64,
    pub current_owner_team_id: i64,
}

/// Returns every draft slot for the season in overall pick order (§7.2.1).
///
/// `lottery_team_order` is the drawn order of the non-playoff teams from
/// [`super::run_lottery`], which fills round 1's first slots.
pub fn compute_draft_order(
    standings: &[league_team_season_standing::Model],
    lottery_team_order: &[i64],
    draft_picks: &[draft_pick::Model],
) -> Result<Vec<DraftSlot>> {
    let (playoff_standings, non_playoff_standings): (Vec<_>, Vec<_>) = standings
        .iter()
        .partition(|standing| standing.made_playoffs);

    ensure!(
        lottery_team_order.len() == non_playoff_standings.len(),
        "lottery drew {} teams but {} teams missed the playoffs.",
        lottery_team_order.len(),
        non_playoff_standings.len()
    );
    for team_id in lottery_team_order {
        ensure!(
            non_playoff_standings
                .iter()
                .any(|standing| standing.team_id == *team_id),
            "lottery team ({team_id}) did not miss the playoffs."
        );
    }

    // (playoff_finish, regular_season_rank, team_id) — the §7.2.1 keys, team_id breaking any tie.
    let mut playoff_seeds = playoff_standings
        .iter()
        .map(|standing| {
            let playoff_finish = standing.playoff_finish.ok_or_else(|| {
                eyre!(
                    "playoff team ({}) has no playoff_finish recorded.",
                    standing.team_id
                )
            })?;
            Ok((
                playoff_finish,
                standing.regular_season_rank,
                standing.team_id,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    // Round 1: worst playoff finish drafts first, champion last; ties go to the worse record.
    playoff_seeds.sort_by_key(|&(playoff_finish, regular_season_rank, team_id)| {
        (
            Reverse(playoff_finish),
            Reverse(regular_season_rank),
            team_id,
        )
    });
    let round_one_teams: Vec<i64> = lottery_team_order
        .iter()
        .copied()
        .chain(playoff_seeds.iter().map(|&(_, _, team_id)| team_id))
        .collect();

    // Rounds 2–5: reverse of round 1's playoff block, so the best finish drafts first there.
    playoff_seeds.sort_by_key(|&(playoff_finish, regular_season_rank, team_id)| {
        (playoff_finish, regular_season_rank, team_id)
    });
    let mut non_playoff_seeds: Vec<_> = non_playoff_standings
        .iter()
        .map(|standing| {
            (
                standing.mid_season_rank,
                standing.regular_season_rank,
                standing.team_id,
            )
        })
        .collect();
    non_playoff_seeds.sort_by_key(|&(mid_season_rank, regular_season_rank, team_id)| {
        (
            Reverse(mid_season_rank),
            Reverse(regular_season_rank),
            team_id,
        )
    });
    let later_round_teams: Vec<i64> = non_playoff_seeds
        .iter()
        .chain(playoff_seeds.iter())
        .map(|&(_, _, team_id)| team_id)
        .collect();

    let picks_by_natural_slot = index_picks_by_natural_slot(draft_picks);

    let mut slots = Vec::with_capacity(round_one_teams.len() * usize::try_from(DRAFT_PICK_ROUNDS)?);
    for round in 1..=DRAFT_PICK_ROUNDS {
        let round_teams = if round == 1 {
            &round_one_teams
        } else {
            &later_round_teams
        };

        for &team_id in round_teams {
            let Some(draft_pick_model) = picks_by_natural_slot.get(&(round, team_id)) else {
                bail!("no round {round} draft pick found for team ({team_id}).");
            };
            slots.push(DraftSlot {
                round,
                draft_pick_id: draft_pick_model.id,
                current_owner_team_id: draft_pick_model.current_owner_team_id,
            });
        }
    }

    Ok(slots)
}

/// Indexes picks by the (round, original owner) slot the draft order is built from.
fn index_picks_by_natural_slot(
    draft_picks: &[draft_pick::Model],
) -> HashMap<(i16, i64), &draft_pick::Model> {
    let mut sorted_picks: Vec<&draft_pick::Model> = draft_picks.iter().collect();
    sorted_picks.sort_by_key(|draft_pick_model| draft_pick_model.id);

    let mut picks_by_natural_slot = HashMap::with_capacity(sorted_picks.len());
    for draft_pick_model in sorted_picks {
        picks_by_natural_slot
            .entry((
                draft_pick_model.round,
                draft_pick_model.original_owner_team_id,
            ))
            .or_insert(draft_pick_model);
    }
    picks_by_natural_slot
}

#[cfg(test)]
mod tests {
    use fbkl_constants::league_rules::DRAFT_PICK_ROUNDS;
    use fbkl_entity::{draft_pick, league_team_season_standing};

    use super::compute_draft_order;

    const END_OF_SEASON_YEAR: i16 = 2025;

    fn standing(
        team_id: i64,
        regular_season_rank: i16,
        mid_season_rank: i16,
        playoff_finish: Option<i16>,
    ) -> league_team_season_standing::Model {
        league_team_season_standing::Model {
            id: team_id,
            league_id: 1,
            team_id,
            end_of_season_year: END_OF_SEASON_YEAR,
            regular_season_rank,
            mid_season_rank,
            made_playoffs: playoff_finish.is_some(),
            playoff_finish,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    /// 12 teams: 1–6 made the playoffs (finish 1..=6), 7–12 did not.
    fn standings() -> Vec<league_team_season_standing::Model> {
        (1..=12)
            .map(|team_id| {
                let rank = i16::try_from(team_id).unwrap();
                let playoff_finish = (team_id <= 6).then_some(rank);
                standing(team_id, rank, rank, playoff_finish)
            })
            .collect()
    }

    /// One pick per team per round, each still held by its original owner.
    fn draft_picks() -> Vec<draft_pick::Model> {
        (1..=DRAFT_PICK_ROUNDS)
            .flat_map(|round| {
                (1..=12).map(move |team_id| draft_pick::Model {
                    id: i64::from(round) * 100 + team_id,
                    round,
                    end_of_season_year: END_OF_SEASON_YEAR,
                    league_id: 1,
                    current_owner_team_id: team_id,
                    original_owner_team_id: team_id,
                    created_at: chrono::Utc::now().into(),
                    updated_at: chrono::Utc::now().into(),
                })
            })
            .collect()
    }

    fn lottery_order() -> Vec<i64> {
        vec![9, 12, 7, 11, 8, 10]
    }

    fn owners_for_round(slots: &[super::DraftSlot], round: i16) -> Vec<i64> {
        slots
            .iter()
            .filter(|slot| slot.round == round)
            .map(|slot| slot.current_owner_team_id)
            .collect()
    }

    #[test]
    fn round_one_follows_lottery_then_playoff_finish() {
        let slots = compute_draft_order(&standings(), &lottery_order(), &draft_picks()).unwrap();

        assert_eq!(
            slots.len(),
            12 * usize::try_from(DRAFT_PICK_ROUNDS).unwrap()
        );
        // Lottery order, then worst playoff finish (6) down to the champion (1) last.
        assert_eq!(
            owners_for_round(&slots, 1),
            vec![9, 12, 7, 11, 8, 10, 6, 5, 4, 3, 2, 1]
        );
    }

    #[test]
    fn later_rounds_share_one_order_worst_mid_season_rank_first() {
        let slots = compute_draft_order(&standings(), &lottery_order(), &draft_picks()).unwrap();

        // Non-playoff teams worst mid-season rank first, then playoff teams best finish first.
        let expected = vec![12, 11, 10, 9, 8, 7, 1, 2, 3, 4, 5, 6];
        for round in 2..=DRAFT_PICK_ROUNDS {
            assert_eq!(owners_for_round(&slots, round), expected);
        }
    }

    #[test]
    fn traded_pick_lands_with_its_current_owner() {
        let mut picks = draft_picks();
        let traded_pick = picks
            .iter_mut()
            .find(|pick| pick.round == 1 && pick.original_owner_team_id == 9)
            .unwrap();
        traded_pick.current_owner_team_id = 3;

        let slots = compute_draft_order(&standings(), &lottery_order(), &picks).unwrap();

        // Team 9 won the lottery, so its slot stays first — but team 3 makes the pick.
        assert_eq!(slots[0].current_owner_team_id, 3);
    }

    #[test]
    fn tied_mid_season_ranks_break_by_record_then_team_id() {
        let mut standings = standings();
        for standing in &mut standings {
            // Teams 7/8/9 all tie at mid-season rank 7; their records differ, 11 and 12 do not.
            match standing.team_id {
                7..=9 => standing.mid_season_rank = 7,
                11..=12 => {
                    standing.mid_season_rank = 11;
                    standing.regular_season_rank = 11;
                }
                _ => {}
            }
        }

        let slots = compute_draft_order(&standings, &lottery_order(), &draft_picks()).unwrap();

        // Worse record drafts first within a tie; team_id breaks a full tie (11 before 12).
        assert_eq!(
            owners_for_round(&slots, 2)[..6].to_vec(),
            vec![11, 12, 10, 9, 8, 7]
        );
    }

    #[test]
    fn rejects_a_lottery_order_that_does_not_match_the_non_playoff_teams() {
        let short_lottery = vec![9, 12, 7];
        assert!(compute_draft_order(&standings(), &short_lottery, &draft_picks()).is_err());

        let playoff_team_in_lottery = vec![1, 12, 7, 11, 8, 10];
        assert!(
            compute_draft_order(&standings(), &playoff_team_in_lottery, &draft_picks()).is_err()
        );
    }
}
