use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ConnectionTrait, TransactionTrait};
use tracing::instrument;

use crate::draft_pick;

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
