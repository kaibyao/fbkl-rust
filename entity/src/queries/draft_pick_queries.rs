use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, LoaderTrait, QueryFilter,
    TransactionTrait,
};
use tracing::instrument;

use crate::{draft_pick, draft_pick_draft_pick_option, draft_pick_option};

#[instrument]
pub async fn insert_draft_pick<C>(
    draft_pick_model: draft_pick::ActiveModel,
    db: &C,
) -> Result<draft_pick::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let inserted_draft_pick_model = draft_pick_model.insert(db).await?;
    Ok(inserted_draft_pick_model)
}

#[instrument]
pub async fn get_draft_picks_affected_by_options<C>(
    draft_pick_options: &[draft_pick_option::Model],
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait + Debug,
{
    let related_draft_picks: Vec<draft_pick::Model> = draft_pick_options
        .load_many_to_many(draft_pick::Entity, draft_pick_draft_pick_option::Entity, db)
        .await?
        .into_iter()
        .flatten()
        .collect();

    Ok(related_draft_picks)
}

#[instrument]
pub async fn get_draft_picks_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait + Debug,
{
    let draft_picks = draft_pick::Entity::find()
        .filter(
            draft_pick::Column::LeagueId
                .eq(league_id)
                .and(draft_pick::Column::EndOfSeasonYear.eq(end_of_season_year)),
        )
        .all(db)
        .await?;

    Ok(draft_picks)
}
