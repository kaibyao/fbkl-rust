use std::fmt::Debug;

use color_eyre::eyre::{ensure, Result};
use contract_entity::ContractType;
use sea_orm::{ActiveValue, ConnectionTrait};

use super::contract_entity;

pub async fn create_rdi_contract_from_rd<C>(
    rd_contract: &contract_entity::Model,
    db: &C,
) -> Result<contract_entity::ActiveModel>
where
    C: ConnectionTrait + Debug,
{
    validate_contract_type(rd_contract)?;
    validate_player_eligibility(rd_contract, db).await?;

    let mut active_model: contract_entity::ActiveModel = rd_contract.clone().into();
    active_model.contract_year = ActiveValue::Set(1);
    active_model.contract_type = ActiveValue::Set(ContractType::RookieDevelopmentInternational);

    Ok(active_model)
}

fn validate_contract_type(rd_contract: &contract_entity::Model) -> Result<()> {
    ensure!(
        rd_contract.contract_type == ContractType::RookieDevelopment,
        "Only RD contracts can be converted to an RDI contract (id = {}).",
        rd_contract.id
    );

    Ok(())
}

async fn validate_player_eligibility<C>(rd_contract: &contract_entity::Model, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let related_player = rd_contract.get_player(db).await?;

    ensure!(related_player.get_is_rdi_eligible(), "Related player ({}) for contract (contract_id = {}) must be eligible to be placed on an international contract.", related_player.get_name(), rd_contract.id);

    Ok(())
}
