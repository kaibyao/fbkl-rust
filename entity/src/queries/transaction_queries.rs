use std::fmt::Debug;

use color_eyre::{eyre::eyre, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use tracing::instrument;

use crate::{
    deadline::{self, DeadlineType},
    transaction::{self, TransactionType},
};

#[instrument]
pub async fn get_or_create_keeper_deadline_transaction<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let maybe_existing_keeper_deadline_transaction = transaction::Entity::find()
        .filter(
            transaction::Column::TransactionType
                .eq(TransactionType::PreseasonKeeper)
                .and(transaction::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(transaction::Column::LeagueId.eq(league_id)),
        )
        .one(db)
        .await?;

    if let Some(existing_keeper_deadline_transaction) = maybe_existing_keeper_deadline_transaction {
        return Ok(existing_keeper_deadline_transaction);
    }

    let maybe_keeper_deadline = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(deadline::Column::DeadlineType.eq(DeadlineType::PreseasonKeeper)),
        )
        .one(db)
        .await?;
    let keeper_deadline = maybe_keeper_deadline.ok_or_else(|| eyre!("Keeper deadline for league ({}) & season end year ({}) not found! Have deadlines for this league been generated?", league_id, end_of_season_year))?;

    let transaction_to_insert =
        transaction::Model::new_keeper_deadline_transaction(&keeper_deadline)?;
    let keeper_deadline_transaction = transaction_to_insert.insert(db).await?;
    Ok(keeper_deadline_transaction)
}
