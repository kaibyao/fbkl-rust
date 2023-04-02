use color_eyre::{eyre::eyre, Result};
use fbkl_constants::league_rules::{
    KEEPER_CONTRACT_COUNT_LIMIT, KEEPER_CONTRACT_TOTAL_SALARY_LIMIT,
};
use fbkl_entity::{
    contract,
    sea_orm::{ConnectionTrait, TransactionTrait},
    team_update,
};

/// Saves the contracts to keep on a team for the season's Keeper Deadline.
pub async fn save_team_keepers<C>(
    team_id: i64,
    contracts: Vec<contract::Model>,
    db: &C,
) -> Result<Vec<team_update::Model>>
where
    C: ConnectionTrait + TransactionTrait,
{
    validate_team_keepers(&contracts)?;
    Ok(vec![])
}

/// Validates the following:
/// * The total contract value is $100 or less.
/// * The total number of non-(RD|RDI|RFA|UFA) keeper contracts is 14 or less.
fn validate_team_keepers(contracts: &[contract::Model]) -> Result<()> {
    let counted_contracts: Vec<&contract::Model> = contracts
        .iter()
        .filter(|contract| match contract.contract_type {
            contract::ContractType::RookieDevelopment => false,
            contract::ContractType::RookieDevelopmentInternational => false,
            contract::ContractType::Rookie => true,
            contract::ContractType::RestrictedFreeAgent => false,
            contract::ContractType::RookieExtension => true,
            contract::ContractType::UnrestrictedFreeAgentOriginalTeam => false,
            contract::ContractType::Veteran => true,
            contract::ContractType::UnrestrictedFreeAgentVeteran => false,
            contract::ContractType::FreeAgent => false,
        })
        .collect();

    if counted_contracts.len() > KEEPER_CONTRACT_COUNT_LIMIT {
        return Err(eyre!("The number of contracts attempted ({}) to be saved as Keepers exceeds the league limit of {}.", counted_contracts.len(), KEEPER_CONTRACT_COUNT_LIMIT));
    }

    let total_counted_contract_value: i16 = counted_contracts
        .iter()
        .map(|contract| contract.salary)
        .sum();
    if total_counted_contract_value > KEEPER_CONTRACT_TOTAL_SALARY_LIMIT {
        return Err(eyre!(
            "The total contract salary amount ({}) exceeds the league salary cap of {}.",
            total_counted_contract_value,
            KEEPER_CONTRACT_TOTAL_SALARY_LIMIT
        ));
    }

    Ok(())
}

// fn get_or_create_keeper_deadline_transaction()
