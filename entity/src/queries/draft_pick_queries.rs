use color_eyre::eyre::{Result, eyre};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, LoaderTrait,
    QueryFilter, QueryOrder, TransactionSession, TransactionTrait,
};
use tracing::instrument;

use crate::{draft_pick, draft_pick_draft_pick_option, draft_pick_option};

#[instrument(skip(db))]
pub async fn insert_draft_pick<C>(
    draft_pick_model: draft_pick::ActiveModel,
    db: &C,
) -> Result<draft_pick::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let inserted_draft_pick_model = draft_pick_model.insert(db).await?;
    Ok(inserted_draft_pick_model)
}

#[instrument(skip(db))]
pub async fn insert_draft_picks<C>(draft_picks: Vec<draft_pick::ActiveModel>, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait,
{
    let transaction = db.begin().await?;
    draft_pick::Entity::insert_many(draft_picks)
        .exec(&transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

#[instrument(skip(db))]
pub async fn get_draft_picks_affected_by_options<C>(
    draft_pick_options: &[draft_pick_option::Model],
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let related_draft_picks: Vec<draft_pick::Model> = draft_pick_options
        .load_many_to_many(draft_pick::Entity, draft_pick_draft_pick_option::Entity, db)
        .await?
        .into_iter()
        .flatten()
        .collect();

    Ok(related_draft_picks)
}

#[instrument(skip(db))]
pub async fn find_draft_pick_by_id<C>(draft_pick_id: i64, db: &C) -> Result<draft_pick::Model>
where
    C: ConnectionTrait,
{
    draft_pick::Entity::find_by_id(draft_pick_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find draft pick ({draft_pick_id})."))
}

#[instrument(skip(db))]
pub async fn get_draft_picks_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<draft_pick::Model>>
where
    C: ConnectionTrait,
{
    let draft_picks = draft_pick::Entity::find()
        .filter(
            draft_pick::Column::LeagueId
                .eq(league_id)
                .and(draft_pick::Column::EndOfSeasonYear.eq(end_of_season_year)),
        )
        .order_by_asc(draft_pick::Column::Round)
        .order_by_asc(draft_pick::Column::Id)
        .all(db)
        .await?;

    Ok(draft_picks)
}
