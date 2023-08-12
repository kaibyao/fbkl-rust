use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    contract::{self, ContractStatus},
    draft_pick,
    draft_pick_option::{self, DraftPickOptionStatus},
    draft_pick_option_amendment::{self, DraftPickOptionAmendmentStatus},
    sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait},
    trade_asset::{self, TradeAssetType},
};
use tracing::instrument;

#[instrument]
pub async fn process_trade_assets<C>(trade_assets: Vec<trade_asset::Model>, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    for trade_asset in trade_assets {
        match trade_asset.asset_type {
            TradeAssetType::Contract => update_trade_asset_contract(&trade_asset, db).await?,
            TradeAssetType::DraftPick => update_trade_asset_draft_pick(&trade_asset, db).await?,
            TradeAssetType::DraftPickOption => {
                update_trade_asset_draft_pick_option(&trade_asset, db).await?
            }
            TradeAssetType::DraftPickOptionAmendment => {
                update_trade_asset_draft_pick_option_amendment(&trade_asset, db).await?
            }
        };
    }

    Ok(())
}

#[instrument]
async fn update_trade_asset_contract<C>(trade_asset: &trade_asset::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let contract_model = trade_asset.get_contract(db).await?;
    let mut new_contract: contract::ActiveModel = contract_model.clone().into();

    new_contract.team_id = ActiveValue::Set(Some(trade_asset.to_team_id));
    new_contract.previous_contract_id = ActiveValue::Set(Some(contract_model.id));
    let mut existing_contract_to_update: contract::ActiveModel = contract_model.into();
    existing_contract_to_update.status = ActiveValue::Set(ContractStatus::Replaced);

    existing_contract_to_update.update(db).await?;
    new_contract.update(db).await?;

    Ok(())
}

#[instrument]
async fn update_trade_asset_draft_pick<C>(trade_asset: &trade_asset::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let draft_pick = trade_asset.get_draft_pick(db).await?;
    let mut draft_pick_to_update: draft_pick::ActiveModel = draft_pick.into();

    draft_pick_to_update.current_owner_team_id = ActiveValue::Set(trade_asset.to_team_id);
    draft_pick_to_update.update(db).await?;

    Ok(())
}

#[instrument]
async fn update_trade_asset_draft_pick_option<C>(
    trade_asset: &trade_asset::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let draft_pick_option = trade_asset.get_draft_pick_option(db).await?;
    let mut draft_pick_option_to_update: draft_pick_option::ActiveModel = draft_pick_option.into();

    draft_pick_option_to_update.status = ActiveValue::Set(DraftPickOptionStatus::Active);
    draft_pick_option_to_update.update(db).await?;

    Ok(())
}

#[instrument]
async fn update_trade_asset_draft_pick_option_amendment<C>(
    trade_asset: &trade_asset::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let amendment = trade_asset.get_draft_pick_option_amendment(db).await?;
    let mut amendment_to_update: draft_pick_option_amendment::ActiveModel = amendment.into();
    amendment_to_update.status = ActiveValue::Set(DraftPickOptionAmendmentStatus::Active);
    amendment_to_update.update(db).await?;

    Ok(())
}
