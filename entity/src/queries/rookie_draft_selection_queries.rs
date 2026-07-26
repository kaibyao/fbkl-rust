use std::fmt::Debug;

use color_eyre::eyre::Result;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, JoinType,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use tracing::instrument;

use crate::{
    contract, draft_pick,
    rookie_draft_selection::{self, RookieDraftSelectionStatus},
};

/// Rookie draft selections made in a league's season, in draft order. The season comes from the
/// selection's draft pick.
#[instrument]
pub async fn find_rookie_draft_selections_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<rookie_draft_selection::Model>>
where
    C: ConnectionTrait + Debug,
{
    let selections = rookie_draft_selection::Entity::find()
        .join(
            JoinType::InnerJoin,
            rookie_draft_selection::Relation::DraftPick.def(),
        )
        .filter(rookie_draft_selection::Column::LeagueId.eq(league_id))
        .filter(draft_pick::Column::EndOfSeasonYear.eq(end_of_season_year))
        .order_by_asc(rookie_draft_selection::Column::Order)
        .all(db)
        .await?;

    Ok(selections)
}

#[instrument]
pub async fn insert_used_rookie_draft_selection<C>(
    signed_rookie_contract: &contract::Model,
    draft_pick_id: i64,
    overall_draft_rank: i16,
    db: &C,
) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + Debug,
{
    let rookie_draft_selection_to_insert = rookie_draft_selection::Model::from_rookie_contract(
        signed_rookie_contract.league_id,
        signed_rookie_contract.id,
        draft_pick_id,
        overall_draft_rank,
    );
    let inserted_rookie_draft_selection = rookie_draft_selection_to_insert.insert(db).await?;
    Ok(inserted_rookie_draft_selection)
}

#[instrument]
pub async fn insert_skipped_rookie_draft_selection<C>(
    league_id: i64,
    draft_pick_id: i64,
    overall_draft_rank: i16,
    db: &C,
) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + Debug,
{
    let rookie_draft_selection_to_insert = rookie_draft_selection::ActiveModel {
        order: ActiveValue::Set(overall_draft_rank),
        status: ActiveValue::Set(RookieDraftSelectionStatus::Skipped),
        contract_id: ActiveValue::NotSet,
        draft_pick_id: ActiveValue::Set(draft_pick_id),
        league_id: ActiveValue::Set(league_id),
        ..Default::default()
    };
    let inserted_rookie_draft_selection = rookie_draft_selection_to_insert.insert(db).await?;
    Ok(inserted_rookie_draft_selection)
}
