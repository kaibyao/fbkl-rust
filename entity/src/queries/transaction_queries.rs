use std::fmt::Debug;

use color_eyre::{eyre::eyre, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use tracing::instrument;

use crate::{
    deadline::{self, DeadlineKind as DeadlineKind},
    transaction::{self, TransactionKind as TransactionKind},
};

#[instrument]
pub async fn get_or_create_keeper_deadline_transaction<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + Debug,
{
    let maybe_existing_keeper_deadline_transaction = transaction::Entity::find()
        .filter(
            transaction::Column::Kind
                .eq(TransactionKind::PreseasonKeeper)
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
                .and(deadline::Column::Kind.eq(DeadlineKind::PreseasonKeeper)),
        )
        .one(db)
        .await?;
    let keeper_deadline = maybe_keeper_deadline.ok_or_else(|| eyre!("Keeper deadline for league ({}) & season end year ({}) not found! Have deadlines for this league been generated?", league_id, end_of_season_year))?;

    let transaction_to_insert =
        transaction::Model::new_keeper_deadline_transaction(&keeper_deadline);
    insert_transaction(transaction_to_insert, db).await
}

/// Creates & inserts a transaction tied to the end of an auction.
#[instrument]
pub async fn insert_auction_transaction<C>(
    deadline_model: &deadline::Model,
    auction_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + Debug,
{
    let transaction_to_insert =
        transaction::Model::new_auction_transaction(deadline_model, auction_id);
    insert_transaction(transaction_to_insert, db).await
}

/// Creates & inserts a transaction tied to a completed trade.
#[instrument]
pub async fn insert_trade_transaction<C>(
    deadline_model: &deadline::Model,
    trade_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + Debug,
{
    let transaction_to_insert = transaction::Model::new_trade_transaction(deadline_model, trade_id);
    insert_transaction(transaction_to_insert, db).await
}

#[instrument]
pub async fn insert_transaction<C>(
    transaction_to_insert: transaction::ActiveModel,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + Debug,
{
    let inserted_transaction = transaction_to_insert.insert(db).await?;
    Ok(inserted_transaction)
}
