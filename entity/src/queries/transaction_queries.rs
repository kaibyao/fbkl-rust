use std::fmt::Debug;

use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, sea_query::Expr,
};
use tracing::instrument;

use crate::{
    auction,
    deadline::{self, DeadlineKind},
    rookie_draft_selection, trade,
    transaction::{self, TransactionKind},
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

/// Creates & inserts a transaction tied to the end of an auction, then points the auction's 1:1 `transaction_id` FK back at it.
#[instrument]
pub async fn insert_auction_transaction<C>(
    deadline_model: &deadline::Model,
    auction_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + Debug,
{
    let transaction_model = insert_transaction(
        transaction::Model::new_auction_transaction(deadline_model),
        db,
    )
    .await?;
    auction::Entity::update_many()
        .col_expr(
            auction::Column::TransactionId,
            Expr::value(transaction_model.id),
        )
        .filter(auction::Column::Id.eq(auction_id))
        .exec(db)
        .await?;
    Ok(transaction_model)
}

/// Creates & inserts a transaction tied to a completed trade, then points the trade's 1:1 `transaction_id` FK back at it.
#[instrument]
pub async fn insert_trade_transaction<C>(
    deadline_model: &deadline::Model,
    trade_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + Debug,
{
    let transaction_model = insert_transaction(
        transaction::Model::new_trade_transaction(deadline_model),
        db,
    )
    .await?;
    trade::Entity::update_many()
        .col_expr(
            trade::Column::TransactionId,
            Expr::value(transaction_model.id),
        )
        .filter(trade::Column::Id.eq(trade_id))
        .exec(db)
        .await?;
    Ok(transaction_model)
}

/// Creates & inserts a transaction tied to a rookie draft selection, then points the selection's 1:1 `transaction_id` FK back at it.
#[instrument]
pub async fn insert_rookie_draft_selection_transaction<C>(
    deadline_model: &deadline::Model,
    rookie_draft_selection_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + Debug,
{
    let transaction_model = insert_transaction(
        transaction::Model::new_rookie_draft_selection_transaction(deadline_model),
        db,
    )
    .await?;
    rookie_draft_selection::Entity::update_many()
        .col_expr(
            rookie_draft_selection::Column::TransactionId,
            Expr::value(transaction_model.id),
        )
        .filter(rookie_draft_selection::Column::Id.eq(rookie_draft_selection_id))
        .exec(db)
        .await?;
    Ok(transaction_model)
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
