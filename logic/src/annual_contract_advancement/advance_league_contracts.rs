use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    contract::{self},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries,
    sea_orm::ConnectionTrait,
    transaction, transaction_queries,
};
use tracing::instrument;

use super::create_team_contracts_for_annual_advancement::create_team_updates_for_advanced_team_contracts;

/// Advances the contracts tied to teams in a league and expires the ones that ended the season as free agents.
#[instrument]
pub async fn advance_league_contracts<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + Debug,
{
    let active_league_contracts =
        contract_queries::find_active_contracts_in_league(league_id, db).await?;

    let mut advanced_contracts = vec![];
    for active_league_contract in active_league_contracts {
        if active_league_contract.kind == contract::ContractKind::FreeAgent {
            // Expire the contracts of players that ended the season as a free agent.
            contract_queries::expire_contract(active_league_contract, db).await?;
        } else {
            // Advance the rest in preparation for Keeper Deadline.
            let advanced_contract =
                contract_queries::advance_contract(active_league_contract, db).await?;
            advanced_contracts.push(advanced_contract);
        }
    }

    let preseason_start_deadline = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::PreseasonStart,
        db,
    )
    .await?;
    let contract_advancement_transaction = transaction_queries::insert_transaction(
        transaction::Model::new_preseason_start_transaction(&preseason_start_deadline),
        db,
    )
    .await?;

    create_team_updates_for_advanced_team_contracts(
        &advanced_contracts,
        &preseason_start_deadline,
        contract_advancement_transaction.id,
        db,
    )
    .await?;

    Ok(advanced_contracts)
}
