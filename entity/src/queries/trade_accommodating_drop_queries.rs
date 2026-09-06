use std::collections::HashSet;

use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use tracing::instrument;

use crate::{trade, trade_accommodating_drop, trade_accommodating_drop::AccommodatingMoveKind};

/// One move an owner submits with a trade, before it has a row of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccommodatingMove {
    pub contract_id: i64,
    pub kind: AccommodatingMoveKind,
}

impl AccommodatingMove {
    /// The contract leaves the roster.
    pub const fn drop_contract(contract_id: i64) -> Self {
        Self {
            contract_id,
            kind: AccommodatingMoveKind::Drop,
        }
    }

    /// The contract goes to the injured reserve instead of being dropped (rules §13.1.5.4).
    pub const fn to_ir(contract_id: i64) -> Self {
        Self {
            contract_id,
            kind: AccommodatingMoveKind::ToIr,
        }
    }
}

/// A contract named as an accommodating move of one trade twice, whichever move each was.
///
/// Concrete (not an opaque `eyre!`) so the resolver can `downcast_ref` and tell the owner which
/// contract was repeated, instead of reporting the unique index's fault as a server error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "contract (id = {contract_id}) is already an accommodating move of trade (id = {trade_id})"
)]
pub struct DuplicateAccommodatingDrop {
    pub trade_id: i64,
    pub contract_id: i64,
}

/// Records the moves one owner submits with a trade, replacing anything that owner submitted
/// before.
///
/// Replacing rather than adding keeps a re-submitted accept from stacking a second copy of the
/// same move onto the trade.
///
/// One contract can accommodate a trade once, whichever move it is and whoever submits it, so a
/// repeat in this owner's own list or a clash with another owner's stored move returns
/// [`DuplicateAccommodatingDrop`].
#[instrument(skip(db))]
pub async fn replace_accommodating_drops<C>(
    trade_id: i64,
    team_id: i64,
    accommodating_moves: &[AccommodatingMove],
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let mut submitted = HashSet::with_capacity(accommodating_moves.len());
    for accommodating_move in accommodating_moves {
        if !submitted.insert(accommodating_move.contract_id) {
            return Err(DuplicateAccommodatingDrop {
                trade_id,
                contract_id: accommodating_move.contract_id,
            }
            .into());
        }
    }

    // Serializes two owners answering one trade, so the clash check below reads the other's rows.
    trade::Entity::find_by_id(trade_id)
        .lock_exclusive()
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find trade with id: {}", trade_id))?;

    trade_accommodating_drop::Entity::delete_many()
        .filter(trade_accommodating_drop::Column::TradeId.eq(trade_id))
        .filter(trade_accommodating_drop::Column::TeamId.eq(team_id))
        .exec(db)
        .await?;

    if accommodating_moves.is_empty() {
        return Ok(());
    }

    let claimed_elsewhere = find_accommodating_drops_for_trade(trade_id, db)
        .await?
        .into_iter()
        .find(|drop| drop.team_id != team_id && submitted.contains(&drop.contract_id));
    if let Some(drop) = claimed_elsewhere {
        return Err(DuplicateAccommodatingDrop {
            trade_id,
            contract_id: drop.contract_id,
        }
        .into());
    }

    trade_accommodating_drop::Entity::insert_many(accommodating_moves.iter().map(
        |accommodating_move| trade_accommodating_drop::ActiveModel {
            id: ActiveValue::NotSet,
            trade_id: ActiveValue::Set(trade_id),
            team_id: ActiveValue::Set(team_id),
            contract_id: ActiveValue::Set(accommodating_move.contract_id),
            kind: ActiveValue::Set(accommodating_move.kind),
        },
    ))
    .exec(db)
    .await?;

    Ok(())
}

/// Every owner's accommodating moves for a trade, oldest first.
///
/// The order is explicit because callers stop at the first bad row: without it Postgres may hand
/// back a different row first after a plan change, and the refusal would name a different contract.
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
        .order_by_asc(trade_accommodating_drop::Column::Id)
        .all(db)
        .await?;

    Ok(drops)
}
