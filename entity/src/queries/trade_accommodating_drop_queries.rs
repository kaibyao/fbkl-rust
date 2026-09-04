use color_eyre::Result;
use sea_orm::{ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use tracing::instrument;

use crate::trade_accommodating_drop;

/// Records the drops one owner submits with a trade, replacing anything that owner submitted before.
///
/// Replacing rather than adding keeps a re-submitted accept from stacking a second copy of the
/// same drop onto the trade.
#[instrument(skip(db))]
pub async fn replace_accommodating_drops<C>(
    trade_id: i64,
    team_id: i64,
    contract_ids: &[i64],
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    trade_accommodating_drop::Entity::delete_many()
        .filter(trade_accommodating_drop::Column::TradeId.eq(trade_id))
        .filter(trade_accommodating_drop::Column::TeamId.eq(team_id))
        .exec(db)
        .await?;

    if contract_ids.is_empty() {
        return Ok(());
    }

    trade_accommodating_drop::Entity::insert_many(contract_ids.iter().map(|contract_id| {
        trade_accommodating_drop::ActiveModel {
            id: ActiveValue::NotSet,
            trade_id: ActiveValue::Set(trade_id),
            team_id: ActiveValue::Set(team_id),
            contract_id: ActiveValue::Set(*contract_id),
        }
    }))
    .exec(db)
    .await?;

    Ok(())
}

/// Every owner's accommodating drops for a trade, oldest first.
#[instrument(skip(db))]
pub async fn find_accommodating_drops_for_trade<C>(
    trade_id: i64,
    db: &C,
) -> Result<Vec<trade_accommodating_drop::Model>>
where
    C: ConnectionTrait,
{
    let drops = trade_accommodating_drop::Entity::find()
        .filter(trade_accommodating_drop::Column::TradeId.eq(trade_id))
        .all(db)
        .await?;

    Ok(drops)
}
