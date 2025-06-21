use std::fmt::Debug;

use color_eyre::eyre::Result;
use fbkl_constants::league_rules::{DRAFT_PICK_ROUNDS, FUTURE_DRAFT_PICK_SEASONS_LIMIT};
use fbkl_entity::{
    draft_pick, draft_pick_queries,
    sea_orm::{ActiveValue, ConnectionTrait, TransactionTrait},
    team_queries,
};
use tracing::instrument;

#[instrument]
pub async fn generate_future_draft_picks<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let end_of_season_year_for_future_draft_picks =
        end_of_season_year + FUTURE_DRAFT_PICK_SEASONS_LIMIT;
    let all_teams_in_league = team_queries::find_teams_in_league(league_id, db).await?;

    // generate all draft picks to insert later
    let mut draft_picks_to_insert = Vec::new();
    for team in all_teams_in_league.iter() {
        for round in 1..=DRAFT_PICK_ROUNDS {
            draft_picks_to_insert.push(draft_pick::ActiveModel {
                id: ActiveValue::NotSet,
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
                league_id: ActiveValue::Set(league_id),
                end_of_season_year: ActiveValue::Set(end_of_season_year_for_future_draft_picks),
                round: ActiveValue::Set(round),
                current_owner_team_id: ActiveValue::Set(team.id),
                original_owner_team_id: ActiveValue::Set(team.id),
            });
        }
    }

    // insert all draft picks
    draft_pick_queries::insert_draft_picks(draft_picks_to_insert, db).await?;

    Ok(())
}
