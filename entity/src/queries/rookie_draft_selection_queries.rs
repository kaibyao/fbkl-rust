use std::fmt::Debug;

use color_eyre::eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait};
use tracing::instrument;

use crate::{
    contract,
    rookie_draft_selection::{self, RookieDraftSelectionStatus},
};

#[instrument]
pub async fn insert_used_rookie_draft_selection<C>(
    signed_rookie_contract: &contract::Model,
    draft_pick_id: i64,
    overall_draft_rank: i16,
    db: &C,
) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + Debug,
{
    let rookie_draft_selection_to_insert = rookie_draft_selection::Model::from_rookie_contract(
        signed_rookie_contract.league_id,
        signed_rookie_contract.id,
        draft_pick_id,
        overall_draft_rank,
    );
    let inserted_rookie_draft_selection = rookie_draft_selection_to_insert.insert(db).await?;
    Ok(inserted_rookie_draft_selection)
}

#[instrument]
pub async fn insert_skipped_rookie_draft_selection<C>(
    league_id: i64,
    draft_pick_id: i64,
    overall_draft_rank: i16,
    db: &C,
) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + Debug,
{
    let rookie_draft_selection_to_insert = rookie_draft_selection::ActiveModel {
        order: ActiveValue::Set(overall_draft_rank),
        status: ActiveValue::Set(RookieDraftSelectionStatus::Skipped),
        contract_id: ActiveValue::NotSet,
        draft_pick_id: ActiveValue::Set(draft_pick_id),
        league_id: ActiveValue::Set(league_id),
        ..Default::default()
    };
    let inserted_rookie_draft_selection = rookie_draft_selection_to_insert.insert(db).await?;
    Ok(inserted_rookie_draft_selection)
}
