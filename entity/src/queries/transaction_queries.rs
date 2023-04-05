use color_eyre::{eyre::eyre, Result};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    TransactionTrait,
};

use crate::{
    deadline::{self, DeadlineType},
    transaction::{self, TransactionType},
};

pub async fn get_or_create_keeper_deadline_transaction<C>(
    league_id: i64,
    season_end_year: i16,
    db: &C,
) -> Result<transaction::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let maybe_existing_keeper_deadline_transaction = transaction::Entity::find()
        .filter(
            transaction::Column::TransactionType
                .eq(TransactionType::PreseasonKeeper)
                .and(transaction::Column::SeasonEndYear.eq(season_end_year))
                .and(transaction::Column::LeagueId.eq(league_id)),
        )
        .one(db)
        .await?;

    if maybe_existing_keeper_deadline_transaction.is_some() {
        return Ok(maybe_existing_keeper_deadline_transaction.unwrap());
    }

    let maybe_keeper_deadline = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::SeasonEndYear.eq(season_end_year))
                .and(deadline::Column::DeadlineType.eq(DeadlineType::PreseasonKeeper)),
        )
        .one(db)
        .await?;
    let keeper_deadline = maybe_keeper_deadline.ok_or_else(|| eyre!("Keeper deadline for league ({}) & season end year ({}) not found! Have deadlines for this league been generated?", league_id, season_end_year))?;

    let transaction_to_insert = transaction::ActiveModel {
        season_end_year: ActiveValue::Set(season_end_year),
        transaction_type: ActiveValue::Set(TransactionType::PreseasonKeeper),
        league_id: ActiveValue::Set(league_id),
        deadline_id: ActiveValue::Set(keeper_deadline.id),
        ..Default::default()
    };
    let keeper_deadline_transaction = transaction_to_insert.insert(db).await?;
    Ok(keeper_deadline_transaction)
}
