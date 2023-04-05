use std::fmt::Debug;

use color_eyre::{eyre::eyre, Result};
use fbkl_constants::league_rules::{
    KEEPER_CONTRACT_COUNT_LIMIT, KEEPER_CONTRACT_TOTAL_SALARY_LIMIT,
};
use fbkl_entity::{
    contract,
    sea_orm::{ConnectionTrait, TransactionTrait},
    team, team_update, team_update_queries, transaction, transaction_queries,
};
use tracing::instrument;

/// Saves the contracts to keep on a team for the season's Keeper Deadline.
#[instrument]
pub async fn save_team_keepers<C>(
    team: &team::Model,
    keeper_contracts: Vec<contract::Model>,
    season_end_year: i16,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    validate_team_keepers(&keeper_contracts)?;

    let league = team.get_league(db).await?;
    let keeper_deadline_transaction =
        transaction_queries::get_or_create_keeper_deadline_transaction(
            league.id,
            season_end_year,
            db,
        )
        .await?;
    let mut keeper_team_updates =
        team_update_queries::find_team_updates_by_transaction(keeper_deadline_transaction.id, db)
            .await?;
    let maybe_existing_keeper_team_update_index = keeper_team_updates
        .iter()
        .position(|team_update| team_update.team_id == team.id);

    match maybe_existing_keeper_team_update_index {
        None => {
            create_new_keeper_team_update(team, &keeper_contracts, &keeper_deadline_transaction, db)
                .await
        }
        Some(existing_keeper_team_update_index) => {
            let existing_keeper_team_update =
                keeper_team_updates.swap_remove(existing_keeper_team_update_index);
            update_existing_keeper_team_update(existing_keeper_team_update, &keeper_contracts, db)
                .await
        }
    }
}

/// Validates the following:
/// * The given list of contracts does not contain any RD, RDI, RFA, or UFA contract.
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

    if counted_contracts.len() != contracts.len() {
        return Err(eyre!("The contracts attempted to be saved as Keepers contained contract types that cannot be kept. Only the following types of contracts can be kept: Rookie (1-3), Rookie Extension (4-5), and Veteran (1-3)."));
    }

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

/// If this is the first time this team is keeping contracts, create new team update + set team update contracts
async fn create_new_keeper_team_update<C>(
    team: &team::Model,
    keeper_contracts: &[contract::Model],
    keeper_transaction: &transaction::Model,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let db_txn = db.begin().await?;
    let create_result = team_update_queries::insert_keeper_team_update(
        team,
        keeper_contracts,
        keeper_transaction,
        db,
    )
    .await;
    db_txn.commit().await?;

    create_result
}

/// If owner previously set keepers, remove previously-saved contracts from team update and save new ones.
async fn update_existing_keeper_team_update<C>(
    keeper_team_update: team_update::Model,
    keeper_contracts: &[contract::Model],
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let db_txn = db.begin().await?;

    let updated_keeper_team_update =
        team_update_queries::update_keeper_team_update(keeper_team_update, keeper_contracts, db)
            .await?;

    db_txn.commit().await?;

    Ok(updated_keeper_team_update)
}
