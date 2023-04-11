use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    contract::{self},
    contract_queries,
    sea_orm::{ConnectionTrait, TransactionTrait},
};
use tracing::instrument;

/// Advances the contracts tied to teams in a league and expires the ones that ended the season as free agents.
#[instrument]
pub async fn advance_team_contracts_for_league<C>(
    league_id: i64,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    println!("Advancing contracts for league {}...", league_id);

    let active_league_contracts =
        contract_queries::find_active_contracts_in_league(league_id, db).await?;

    let db_txn = db.begin().await?;

    let mut expired_contracts = vec![];
    let mut advanced_contracts = vec![];
    for active_league_contract in active_league_contracts {
        if active_league_contract.contract_type == contract::ContractType::FreeAgent {
            // Expire the contracts of players that ended the season as a free agent.
            let contract_updated_to_expired =
                contract_queries::expire_contract(active_league_contract, &db_txn).await?;
            expired_contracts.push(contract_updated_to_expired);
        } else {
            // Advance the rest in preparation for Keeper Deadline.

            let (_updated_original_contract, advanced_contract) =
                contract_queries::advance_contract(active_league_contract, &db_txn).await?;
            advanced_contracts.push(advanced_contract);
        }
    }

    db_txn.commit().await?;

    println!(
        "{} contracts advanced in league {}",
        advanced_contracts.len(),
        league_id
    );
    Ok(advanced_contracts)
}
