use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};
use tracing::instrument;

use crate::deadline::{self, DeadlineType};

#[instrument]
pub async fn find_deadline_for_season_by_type<C>(
    league_id: i64,
    end_of_season_year: i16,
    deadline_type: DeadlineType,
    db: &C,
) -> Result<Option<deadline::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let maybe_deadline_model = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(deadline::Column::DeadlineType.eq(deadline_type)),
        )
        .one(db)
        .await?;
    Ok(maybe_deadline_model)
}
