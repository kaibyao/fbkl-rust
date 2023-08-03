use std::{collections::HashMap, fmt::Debug};

use color_eyre::Result;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait,
};
use tracing::instrument;

use crate::team;

#[instrument]
pub async fn find_teams_in_league<C>(league_id: i64, db: &C) -> Result<Vec<team::Model>>
where
    C: ConnectionTrait + Debug,
{
    let team_models = team::Entity::find()
        .join(JoinType::LeftJoin, team::Relation::League.def())
        .filter(team::Column::LeagueId.eq(league_id))
        .all(db)
        .await?;
    Ok(team_models)
}

#[instrument]
pub async fn find_teams_by_name_in_league<C>(
    league_id: i64,
    db: &C,
) -> Result<HashMap<String, team::Model>>
where
    C: ConnectionTrait + Debug,
{
    let teams_by_name = find_teams_in_league(league_id, db)
        .await?
        .into_iter()
        .map(|team| (team.name.clone(), team))
        .collect();
    Ok(teams_by_name)
}
