use std::fmt::Debug;

use color_eyre::{eyre::bail, Result};
use fbkl_constants::league_rules::{
    KEEPER_CONTRACT_COUNT_LIMIT, KEEPER_CONTRACT_TOTAL_SALARY_LIMIT,
};
use fbkl_entity::{
    contract::{self, ContractType},
    sea_orm::ConnectionTrait,
    team,
    team_update::{self},
    team_update_queries, transaction, transaction_queries,
};
use tracing::instrument;

/// Saves the contracts to keep on a team for the season's Keeper Deadline, while also dropping non-keeper contracts.
#[instrument]
pub async fn save_keeper_team_update<C>(
    team_model: &team::Model,
    keeper_contracts: Vec<contract::Model>,
    end_of_season_year: i16,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    validate_team_keepers(&keeper_contracts)?;

    let league = team_model.get_league(db).await?;
    let keeper_deadline_transaction =
        transaction_queries::get_or_create_keeper_deadline_transaction(
            league.id,
            end_of_season_year,
            db,
        )
        .await?;
    let mut keeper_team_updates =
        team_update_queries::find_team_updates_by_transaction(keeper_deadline_transaction.id, db)
            .await?;
    let maybe_existing_keeper_team_update_index = keeper_team_updates
        .iter()
        .position(|team_update| team_update.team_id == team_model.id);

    match maybe_existing_keeper_team_update_index {
        None => {
            create_new_keeper_team_update(
                team_model,
                &keeper_contracts,
                &keeper_deadline_transaction,
                db,
            )
            .await
        }
        Some(existing_keeper_team_update_index) => {
            let existing_keeper_team_update =
                keeper_team_updates.swap_remove(existing_keeper_team_update_index);
            update_existing_keeper_team_update(
                team_model,
                existing_keeper_team_update,
                &keeper_contracts,
                db,
            )
            .await
        }
    }
}

/// Validates the following:
/// * The given list of contracts does not contain any RFA or UFA contract.
/// * The total contract value is $100 or less.
/// * The total number of non-(RFA|UFA) keeper contracts is 14 or less.
#[instrument]
fn validate_team_keepers(contracts: &[contract::Model]) -> Result<()> {
    let counted_contracts: Vec<&contract::Model> = contracts
        .iter()
        .filter(|contract| match contract.contract_type {
            ContractType::RookieDevelopment => true,
            ContractType::RookieDevelopmentInternational => true,
            ContractType::Rookie => true,
            ContractType::RestrictedFreeAgent => false,
            ContractType::RookieExtension => true,
            ContractType::UnrestrictedFreeAgentOriginalTeam => false,
            ContractType::Veteran => true,
            ContractType::UnrestrictedFreeAgentVeteran => false,
            ContractType::FreeAgent => false,
        })
        .collect();

    if counted_contracts.len() != contracts.len() {
        bail!("The contracts attempted to be saved as Keepers contained contract types that cannot be kept. Only the following types of contracts can be kept: Rookie Development (International) (1-3), Rookie (1-3), Rookie Extension (4-5), and Veteran (1-3).\n\nGiven contracts:\n{:#?}", contracts);
    }

    let counted_non_rd_contracts: Vec<&contract::Model> = counted_contracts
        .into_iter()
        .filter(|contract| {
            contract.contract_type != ContractType::RookieDevelopment
                && contract.contract_type != ContractType::RookieDevelopmentInternational
        })
        .collect();

    if counted_non_rd_contracts.len() > KEEPER_CONTRACT_COUNT_LIMIT {
        bail!("The number of contracts attempted ({}) to be saved as Keepers exceeds the league limit of {}.\n\nContracts: {:#?}", counted_non_rd_contracts.len(), KEEPER_CONTRACT_COUNT_LIMIT, counted_non_rd_contracts);
    }

    let total_counted_contract_value: i16 = counted_non_rd_contracts
        .iter()
        .map(|contract| contract.salary)
        .sum();
    if total_counted_contract_value > KEEPER_CONTRACT_TOTAL_SALARY_LIMIT {
        bail!(
            "The total contract salary amount ({}) exceeds the league salary cap of {}.",
            total_counted_contract_value,
            KEEPER_CONTRACT_TOTAL_SALARY_LIMIT
        );
    }

    Ok(())
}

/// If this is the first time this team is keeping contracts, create new team update + set team update contracts
#[instrument]
async fn create_new_keeper_team_update<C>(
    team: &team::Model,
    keeper_contracts: &[contract::Model],
    keeper_transaction: &transaction::Model,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    team_update_queries::insert_keeper_team_update(team, keeper_contracts, keeper_transaction, db)
        .await
}

/// If owner previously set keepers, remove previously-saved contracts from team update and save new ones.
#[instrument]
async fn update_existing_keeper_team_update<C>(
    team_model: &team::Model,
    keeper_team_update: team_update::Model,
    keeper_contracts: &[contract::Model],
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let updated_keeper_team_update = team_update_queries::update_keeper_team_update(
        team_model,
        keeper_team_update,
        keeper_contracts,
        db,
    )
    .await?;

    Ok(updated_keeper_team_update)
}
