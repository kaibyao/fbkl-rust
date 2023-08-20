use std::fmt::Debug;

use color_eyre::{eyre::ensure, Result};
use fbkl_entity::{
    contract, contract_queries, draft_pick,
    draft_pick_option::{self, DraftPickOptionStatus},
    sea_orm::ConnectionTrait,
    trade_asset,
};
use tracing::instrument;

use super::process_trade::TradeAssetRelatedModelCache;

#[instrument]
pub async fn validate_trade_assets<C>(
    trade_asset_related_models: &TradeAssetRelatedModelCache,
    trade_id: i64,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    ensure!(
        !trade_asset_related_models
            .trade_asset_contracts_by_trade_asset_id
            .is_empty()
            && !trade_asset_related_models
                .trade_asset_draft_picks_by_trade_asset_id
                .is_empty()
            && !trade_asset_related_models
                .trade_asset_draft_pick_options_by_trade_asset_id
                .is_empty(),
        "Cannot process a trade with no trade assets."
    );

    for (_trade_asset_id, (trade_asset, contract)) in trade_asset_related_models
        .trade_asset_contracts_by_trade_asset_id
        .iter()
    {
        validate_contract_trade_asset(trade_asset, contract, trade_id, db).await?;
    }
    for (_trade_asset_id, (trade_asset, draft_pick)) in trade_asset_related_models
        .trade_asset_draft_picks_by_trade_asset_id
        .iter()
    {
        validate_draft_pick_trade_asset(trade_asset, draft_pick, trade_id, db)?;
    }
    for (_trade_asset_id, (trade_asset, draft_pick_option)) in trade_asset_related_models
        .trade_asset_draft_pick_options_by_trade_asset_id
        .iter()
    {
        validate_draft_pick_option_trade_asset(trade_asset, draft_pick_option, db)?;
    }

    Ok(())
}

#[instrument]
async fn validate_contract_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    contract_model: &contract::Model,
    trade_id: i64,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    // Ensure it's still the latest contract.
    contract_queries::validate_contract_is_latest_in_chain(contract_model, db).await?;

    // Ensure contract is owned by the team referenced by the trade asset entity
    ensure!(contract_model.team_id.map_or(false, |team_id| team_id == trade_asset_model.from_team_id), "Contract ({}) is not currently owned by the team listed as the owning team in this trade ({}). Contract's owning team = {:?}. Trade's listed owning team = {}.", contract_model.id, trade_id, contract_model.team_id, trade_asset_model.from_team_id);

    Ok(())
}

#[instrument]
fn validate_draft_pick_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    draft_pick_model: &draft_pick::Model,
    trade_id: i64,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    // Ensure draft pick is owned by the team referenced in the trade asset entity
    ensure!(draft_pick_model.current_owner_team_id == trade_asset_model.from_team_id, "Draft pick ({}) is not currently owned by the team listed as the owning team in this trade ({}). Draft pick's owning team = {:?}. Trade's listed owning team = {}.", draft_pick_model.id, trade_id, draft_pick_model.current_owner_team_id, trade_asset_model.from_team_id);

    Ok(())
}

#[instrument]
fn validate_draft_pick_option_trade_asset<C>(
    trade_asset_model: &trade_asset::Model,
    draft_pick_option: &draft_pick_option::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    // Ensure that the draft pick option is in a proposed status, as they can't be traded after creation.
    ensure!(
        draft_pick_option.status == DraftPickOptionStatus::Proposed,
        "Draft pick option (id={}) must have a `Proposed` status (owners can only trade a new draft pick option). (status = {:?})",
        draft_pick_option.id,
        draft_pick_option.status
    );

    Ok(())
}
