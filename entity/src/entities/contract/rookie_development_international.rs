use std::fmt::Debug;

use color_eyre::eyre::{ensure, Result};
use contract_entity::ContractKind;
use sea_orm::{ActiveValue, ConnectionTrait};

use super::contract_entity;

pub fn create_rd_contract_from_rdi(
    rdi_contract: &contract_entity::Model,
) -> Result<contract_entity::ActiveModel> {
    validate_contract_type_for_rdi_to_rd(rdi_contract)?;

    let mut active_model: contract_entity::ActiveModel = rdi_contract.clone().into();
    active_model.id = ActiveValue::NotSet;
    active_model.year_number = ActiveValue::Set(1);
    active_model.kind = ActiveValue::Set(ContractKind::RookieDevelopment);

    Ok(active_model)
}

pub async fn create_rdi_contract_from_rd<C>(
    rd_contract: &contract_entity::Model,
    db: &C,
) -> Result<contract_entity::ActiveModel>
where
    C: ConnectionTrait + Debug,
{
    validate_contract_type_for_rd_to_rdi(rd_contract)?;
    validate_player_eligibility(rd_contract, db).await?;

    let mut active_model: contract_entity::ActiveModel = rd_contract.clone().into();
    active_model.id = ActiveValue::NotSet;
    active_model.year_number = ActiveValue::Set(1);
    active_model.kind = ActiveValue::Set(ContractKind::RookieDevelopmentInternational);

    Ok(active_model)
}

fn validate_contract_type_for_rd_to_rdi(rd_contract: &contract_entity::Model) -> Result<()> {
    ensure!(
        rd_contract.kind == ContractKind::RookieDevelopment,
        "Only RD contracts can be converted to an RDI contract (id = {}).",
        rd_contract.id
    );

    Ok(())
}

fn validate_contract_type_for_rdi_to_rd(rdi_contract: &contract_entity::Model) -> Result<()> {
    ensure!(
        rdi_contract.kind == ContractKind::RookieDevelopmentInternational,
        "Only RDI contracts can be converted to an RD contract (id = {}).",
        rdi_contract.id
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
