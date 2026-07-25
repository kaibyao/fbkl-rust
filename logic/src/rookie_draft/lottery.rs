//! The rookie draft lottery for first-round picks 1-6 (§7.2.4-§7.2.5).
//!
//! The six non-playoff teams get 6/5/4/3/2/1 balls by mid-season rank (worst gets the most) and
//! picks 1 through 6 are drawn sequentially, each winner leaving the pool before the next draw.
//! Balls belong to the *standings slot*, so a non-playoff team that traded its first-rounder away
//! still supplies the odds — the resulting pick just lands with the current owner.
//!
//! The draw is a seeded `StdRng`, and the seed is committed to the database before the draw is
//! revealed, so any owner can re-run it and confirm the order was not re-rolled.

use std::{cmp::Reverse, fmt::Write as _};

use color_eyre::{Result, eyre::ensure};
use fbkl_constants::league_rules::ROOKIE_DRAFT_LOTTERY_BALLS;
use fbkl_entity::{
    league_team_season_standing,
    rookie_draft_lottery_queries::{self, NewRookieDraftLotteryPick},
    sea_orm::ConnectionTrait,
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use tracing::instrument;

/// Runs (or replays) the league season's lottery, returning the drawn non-playoff team ids in
/// pick order.
///
/// Already-run lotteries are replayed from the stored draw rather than re-rolled, so a second call
/// from the scheduler or a commissioner cannot change the order.
#[instrument(skip(standings))]
pub async fn run_lottery<C>(
    league_id: i64,
    end_of_season_year: i16,
    standings: &[league_team_season_standing::Model],
    db: &C,
) -> Result<Vec<i64>>
where
    C: ConnectionTrait + std::fmt::Debug,
{
    if let Some(existing_lottery) = rookie_draft_lottery_queries::find_lottery_for_league_season(
        league_id,
        end_of_season_year,
        db,
    )
    .await?
    {
        let existing_picks =
            rookie_draft_lottery_queries::find_lottery_picks(existing_lottery.id, db).await?;
        if !existing_picks.is_empty() {
            return Ok(existing_picks
                .into_iter()
                .map(|pick| pick.team_id)
                .collect());
        }
    }

    let seed = rand::random::<i64>();
    let lottery_model =
        rookie_draft_lottery_queries::insert_lottery_seed(league_id, end_of_season_year, seed, db)
            .await?;

    // The committed seed wins: a crash between commit and reveal replays the same draw.
    let (drawn_picks, rng_log) = draw_lottery(standings, lottery_model.rng_seed)?;
    let team_order = drawn_picks.iter().map(|pick| pick.team_id).collect();
    rookie_draft_lottery_queries::save_lottery_draw(lottery_model.id, drawn_picks, rng_log, db)
        .await?;

    Ok(team_order)
}

/// Draws picks 1-6 from the seeded ball pool, returning the drawn slots and the audit log.
fn draw_lottery(
    standings: &[league_team_season_standing::Model],
    seed: i64,
) -> Result<(Vec<NewRookieDraftLotteryPick>, String)> {
    // Worst mid-season rank first, ties by worse record then team_id, matching `compute_draft_order`.
    let mut non_playoff_standings: Vec<_> = standings
        .iter()
        .filter(|standing| !standing.made_playoffs)
        .collect();
    non_playoff_standings.sort_by_key(|standing| {
        (
            Reverse(standing.mid_season_rank),
            Reverse(standing.regular_season_rank),
            standing.team_id,
        )
    });
    ensure!(
        non_playoff_standings.len() == ROOKIE_DRAFT_LOTTERY_BALLS.len(),
        "the lottery needs {} non-playoff teams but the standings have {}.",
        ROOKIE_DRAFT_LOTTERY_BALLS.len(),
        non_playoff_standings.len()
    );

    let mut pool: Vec<(i64, u32)> = non_playoff_standings
        .iter()
        .zip(ROOKIE_DRAFT_LOTTERY_BALLS)
        .map(|(standing, balls)| (standing.team_id, balls))
        .collect();

    let mut rng = StdRng::seed_from_u64(seed.cast_unsigned());
    let mut drawn_picks = Vec::with_capacity(pool.len());
    let mut rng_log = format!("seed={seed}\n");

    for pick_number in 1..=i16::try_from(ROOKIE_DRAFT_LOTTERY_BALLS.len())? {
        let total_balls: u32 = pool.iter().map(|&(_, balls)| balls).sum();
        let mut drawn_ball = rng.gen_range(0..total_balls);
        // The drawn ball is below the pool total, so it always lands inside one team's ball range.
        let mut winning_index = pool.len() - 1;
        for (index, &(_, balls)) in pool.iter().enumerate() {
            if drawn_ball < balls {
                winning_index = index;
                break;
            }
            drawn_ball -= balls;
        }
        let (team_id, balls_held) = pool.remove(winning_index);

        writeln!(
            rng_log,
            "pick {pick_number}: team {team_id} ({balls_held}/{total_balls} balls)"
        )?;
        drawn_picks.push(NewRookieDraftLotteryPick {
            pick_number,
            team_id,
            balls_held: i16::try_from(balls_held)?,
        });
    }

    Ok((drawn_picks, rng_log))
}

#[cfg(test)]
mod tests {
    use fbkl_entity::league_team_season_standing;

    use super::draw_lottery;

    /// 12 teams: 1-6 made the playoffs, 7-12 did not (12 is the worst, so it holds 6 balls).
    fn standings() -> Vec<league_team_season_standing::Model> {
        (1..=12)
            .map(|team_id| {
                let rank = i16::try_from(team_id).unwrap();
                league_team_season_standing::Model {
                    id: team_id,
                    league_id: 1,
                    team_id,
                    end_of_season_year: 2025,
                    regular_season_rank: rank,
                    mid_season_rank: rank,
                    made_playoffs: team_id <= 6,
                    playoff_finish: (team_id <= 6).then_some(rank),
                    created_at: chrono::Utc::now().into(),
                    updated_at: chrono::Utc::now().into(),
                }
            })
            .collect()
    }

    #[test]
    fn draw_is_reproducible_for_a_seed() {
        let (first_draw, first_log) = draw_lottery(&standings(), 42).unwrap();
        let (second_draw, second_log) = draw_lottery(&standings(), 42).unwrap();

        let team_order: Vec<i64> = first_draw.iter().map(|pick| pick.team_id).collect();
        assert_eq!(team_order.len(), 6);
        assert_eq!(
            team_order,
            second_draw
                .iter()
                .map(|pick| pick.team_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(first_log, second_log);
        // Every non-playoff team is drawn exactly once, worst team holding 6 balls.
        let mut sorted_teams = team_order;
        sorted_teams.sort_unstable();
        assert_eq!(sorted_teams, vec![7, 8, 9, 10, 11, 12]);
        assert_eq!(
            first_draw
                .iter()
                .find(|pick| pick.team_id == 12)
                .unwrap()
                .balls_held,
            6
        );
    }

    #[test]
    fn ball_weighting_favours_the_worst_team() {
        let standings = standings();
        let mut first_pick_wins = [0_u32; 13];
        for seed in 0..600 {
            let (draw, _) = draw_lottery(&standings, seed).unwrap();
            first_pick_wins[usize::try_from(draw[0].team_id).unwrap()] += 1;
        }

        // 6 balls vs 1: the worst team should win pick 1 far more often than the best non-playoff team.
        assert!(first_pick_wins[12] > first_pick_wins[7] * 2);
    }

    #[test]
    fn rejects_standings_without_six_non_playoff_teams() {
        let mut standings = standings();
        standings.retain(|standing| standing.team_id != 12);
        assert!(draw_lottery(&standings, 1).is_err());
    }
}
