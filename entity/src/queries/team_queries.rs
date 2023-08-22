use std::{collections::HashMap, fmt::Debug};

use color_eyre::{eyre::eyre, Result};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, JoinType, LoaderTrait, QueryFilter, QuerySelect,
    RelationTrait,
};
use tracing::instrument;

use crate::{team, team_user, trade_action};

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

/// Finds the teams related to the given trade actions and returns a map of trade_action id to its related team.
#[instrument]
pub async fn find_teams_by_trade_actions<C>(
    trade_actions: &[trade_action::Model],
    db: &C,
) -> Result<HashMap<i64, team::Model>>
where
    C: ConnectionTrait + Debug,
{
    let trade_action_teams = trade_actions
        .load_many_to_many(team::Entity, team_user::Entity, db)
        .await?;
    let mut teams_by_trade_action: HashMap<i64, team::Model> = HashMap::new();

    for (trade_action, mut maybe_trade_action_teams) in
        trade_actions.iter().zip(trade_action_teams.into_iter())
    {
        // There is only ever 1 team per trade action.
        let trade_action_team = maybe_trade_action_teams.pop().ok_or_else(|| {
            eyre!(
                "Could not find team related to trade action: {}.",
                trade_action.id
            )
        })?;
        teams_by_trade_action.insert(trade_action.id, trade_action_team);
    }

    Ok(teams_by_trade_action)
}
