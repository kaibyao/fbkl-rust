use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, JoinType, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, sea_query::Expr,
};
use tracing::instrument;

use crate::{
    auction,
    deadline::{self, DeadlineKind},
    queries::pagination::{Paged, fetch_page},
    rookie_draft_selection, team_update, trade,
    transaction::{self, TransactionKind},
};

#[instrument(skip(db))]
pub async fn find_transaction_by_id<C>(transaction_id: i64, db: &C) -> Result<transaction::Model>
where
    C: ConnectionTrait,
{
    transaction::Entity::find_by_id(transaction_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find transaction with id: {transaction_id}"))
}

/// One page of a league's transaction audit feed, newest first, optionally narrowed to a single
/// team or `TransactionKind`. The feed spans the league's whole history, so it is never unbounded.
///
/// The team filter joins `team_update` (a transaction carries no `team_id` of its own), which is
/// why the select is `DISTINCT` — a transaction touching both sides of a trade has two updates.
#[instrument(skip(db))]
pub async fn find_transactions_in_league<C>(
    league_id: i64,
    maybe_team_id: Option<i64>,
    maybe_kind: Option<TransactionKind>,
    page: u64,
    page_size: u64,
    db: &C,
) -> Result<Paged<transaction::Model>>
where
    C: ConnectionTrait,
{
    let mut query = transaction::Entity::find()
        .filter(transaction::Column::LeagueId.eq(league_id))
        .order_by_desc(transaction::Column::Id);

    if let Some(kind) = maybe_kind {
        query = query.filter(transaction::Column::Kind.eq(kind));
    }

    if let Some(team_id) = maybe_team_id {
        query = query
            .join(JoinType::InnerJoin, transaction::Relation::TeamUpdate.def())
            .filter(team_update::Column::TeamId.eq(team_id))
            .distinct();
    }

    fetch_page(query, page, page_size, db).await
}

/// The league's keeper transaction for a season, if keepers have been touched at all yet.
#[instrument(skip(db))]
pub async fn find_keeper_deadline_transaction<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Option<transaction::Model>>
where
    C: ConnectionTrait,
{
    let found = transaction::Entity::find()
        .filter(
            transaction::Column::Kind
                .eq(TransactionKind::PreseasonKeeper)
                .and(transaction::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(transaction::Column::LeagueId.eq(league_id)),
        )
        .one(db)
        .await?;

    Ok(found)
}

#[instrument(skip(db))]
pub async fn get_or_create_keeper_deadline_transaction<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait,
{
    let maybe_existing_keeper_deadline_transaction =
        find_keeper_deadline_transaction(league_id, end_of_season_year, db).await?;

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
#[instrument(skip(db))]
pub async fn insert_auction_transaction<C>(
    deadline_model: &deadline::Model,
    auction_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait,
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
#[instrument(skip(db))]
pub async fn insert_trade_transaction<C>(
    deadline_model: &deadline::Model,
    trade_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait,
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
#[instrument(skip(db))]
pub async fn insert_rookie_draft_selection_transaction<C>(
    deadline_model: &deadline::Model,
    rookie_draft_selection_id: i64,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait,
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

#[instrument(skip(db))]
pub async fn insert_transaction<C>(
    transaction_to_insert: transaction::ActiveModel,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait,
{
    let inserted_transaction = transaction_to_insert.insert(db).await?;
    Ok(inserted_transaction)
}
