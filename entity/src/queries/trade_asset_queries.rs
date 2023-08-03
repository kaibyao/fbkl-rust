use std::fmt::Debug;

use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, TransactionTrait};
use tracing::instrument;

use crate::{
    contract::{self, ContractStatus},
    trade, trade_asset,
};

/// Creates a new, not-yet-inserted trade asset from a given contract, without a set `trade_id`.
#[instrument]
pub fn new_trade_asset_active_model_from_contract(
    contract_model: &contract::Model,
    from_team_id: i64,
    to_team_id: i64,
) -> Result<trade_asset::ActiveModel> {
    validate_contract_trade_asset(contract_model, from_team_id)?;

    let trade_asset_active_model =
        trade_asset::Model::from_contract(None, contract_model.id, from_team_id, to_team_id);

    Ok(trade_asset_active_model)
}

/// Inserts a new trade (contract) asset for a trade.
#[instrument]
pub async fn insert_trade_asset_from_contract<C>(
    trade_model: &trade::Model,
    contract_model: &contract::Model,
    from_team_id: i64,
    to_team_id: i64,
    db: &C,
) -> Result<trade_asset::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let trade_asset_model_to_insert =
        new_trade_asset_active_model_from_contract(contract_model, from_team_id, to_team_id)?;

    let inserted_trade_asset = trade_asset_model_to_insert.insert(db).await?;

    Ok(inserted_trade_asset)
}

fn validate_contract_trade_asset(
    contract_model: &contract::Model,
    from_team_id: i64,
) -> Result<()> {
    if contract_model.status != ContractStatus::Active {
        bail!(
            "Cannot trade an expired or replaced contract (id = {})",
            contract_model.id
        );
    }

    let contract_team_id = contract_model
        .team_id
        .ok_or_else(|| eyre!("Contract is missing a team_id (id = {})", contract_model.id))?;
    if contract_team_id != from_team_id {
        bail!("Contract's owning team and trade asset's sending team do not match. contract.team_id = {}. trade_asset.from_team_id = {}.", contract_team_id, from_team_id);
    }

    // let teams_involved_in_trade_models = trade_model
    //     .find_linked(TeamsInvolvedInTrade)
    //     .all(db)
    //     .await?;
    // let trade_team_ids: Vec<i64> = teams_involved_in_trade_models
    //     .iter()
    //     .map(|team_model| team_model.id)
    //     .collect();
    // if !trade_team_ids.contains(&from_team_id) {
    //     bail!(
    //         "Trade asset's sending team is not involved in this trade. trade_id = {}. involved team_ids = {}. trade_asset.from_team_id = {}.",
    //         trade_model.id,
    //         trade_team_ids.iter().map(|team_id| team_id.to_string()).collect::<Vec<_>>().join(", "),
    //         from_team_id
    //     );
    // }
    // if !trade_team_ids.contains(&to_team_id) {
    //     bail!(
    //         "Trade asset's receiving team is not involved in this trade. trade_id = {}. involved team_ids = {}. trade_asset.to_team_id = {}.",
    //         trade_model.id,
    //         trade_team_ids.iter().map(|team_id| team_id.to_string()).collect::<Vec<_>>().join(", "),
    //         to_team_id
    //     );
    // }

    Ok(())
}
