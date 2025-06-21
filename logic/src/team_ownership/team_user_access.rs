use std::fmt::Debug;

use color_eyre::eyre::Result;
use fbkl_entity::{
    sea_orm::ConnectionTrait, team, team_user::LeagueRole,
    team_user_queries::get_all_team_users_by_user_and_league,
};

pub async fn get_team_user_access_for_user_in_league<C>(
    user_id: i64,
    league_id: i64,
    db: &C,
) -> Result<Option<team::Model>>
where
    C: ConnectionTrait + Debug,
{
    let all_league_team_users_for_user =
        get_all_team_users_by_user_and_league(&user_id, &league_id, db).await?;
    let current_owner_team =
        all_league_team_users_for_user
            .iter()
            .find_map(|(team_user, maybe_team)| {
                if team_user.league_role == LeagueRole::TeamOwner {
                    maybe_team.to_owned()
                } else {
                    None
                }
            });
    Ok(current_owner_team)
}
