use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use fbkl_entity::{
    contract, contract_queries,
    deadline::DeadlineType,
    deadline_queries,
    sea_orm::{ConnectionTrait, ModelTrait, TransactionTrait},
    team_update::{self, ContractUpdateType, TeamUpdateData, TeamUpdateStatus},
    team_update_queries, transaction,
};
use std::{collections::HashMap, fmt::Debug};
use tracing::{info, instrument};

/// Processes the team_updates that have been created for the Keeper Deadline and sets the status for them.
pub async fn process_keeper_deadline_transaction<C>(
    league_id: i64,
    season_end_year: i16,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let (keeper_team_updates, mut active_league_contracts_by_id) =
        validate_and_get_process_keeper_deadline_data(league_id, season_end_year, db).await?;

    let mut total_contracts_kept = 0;
    let mut total_contracts_dropped = 0;

    for team_update_model in keeper_team_updates {
        let team_id = team_update_model.team_id;
        let status_set_to_in_progress = team_update_queries::update_team_update_status(
            team_update_model,
            TeamUpdateStatus::InProgress,
            db,
        )
        .await?;

        match process_keeper_deadline_transaction_inner(
            status_set_to_in_progress.clone(),
            &mut active_league_contracts_by_id,
            db,
        )
        .await
        {
            Ok((num_keepers, num_dropped)) => {
                total_contracts_kept += num_keepers;
                total_contracts_dropped += num_dropped;

                team_update_queries::update_team_update_status(
                    status_set_to_in_progress,
                    TeamUpdateStatus::Done,
                    db,
                )
                .await?;

                info!(team_id, num_keepers, num_dropped);
            }
            Err(e) => {
                team_update_queries::update_team_update_status(
                    status_set_to_in_progress,
                    TeamUpdateStatus::Error,
                    db,
                )
                .await?;
                bail!(e);
            }
        }
    }

    info!(total_contracts_kept, total_contracts_dropped);

    Ok(())
}

/// Processes the team_updates that have been created for the Keeper Deadline. Returns a tuple containing the number of contracts kept and dropped.
#[instrument]
async fn process_keeper_deadline_transaction_inner<C>(
    team_update_model: team_update::Model,
    active_league_contracts_by_id: &mut HashMap<i64, contract::Model>,
    db: &C,
) -> Result<(i16, i16)>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let mut num_contracts_kept = 0;
    let mut num_contracts_dropped = 0;

    let team_update_data = team_update_model.get_data()?;
    match team_update_data {
        TeamUpdateData::Settings(_) => bail!("Expected Keeper Deadline team update to be a roster update, but got settings instead: {:#?}", team_update_data),
        TeamUpdateData::Roster(contract_updates) => {
            for contract_update in contract_updates {
                match contract_update.update_type {
                    ContractUpdateType::Keeper => {
                        num_contracts_kept += 1;
                    },
                    ContractUpdateType::Drop => {
                        let related_league_contract = active_league_contracts_by_id.remove(&contract_update.contract_id).ok_or_else(|| eyre!("Contract referred by keeper deadline team update is not an active contract or wasn't found. contract_id: {}", contract_update.contract_id))?;
                        contract_queries::drop_contract(related_league_contract, true, db).await?;

                        num_contracts_dropped += 1;
                    },
                    x => bail!("Did not expect contract update type while processing keeper deadline: {:#?}", x),
                }
            }
        }
    };

    Ok((num_contracts_kept, num_contracts_dropped))
}

#[instrument]
async fn validate_and_get_process_keeper_deadline_data<C>(
    league_id: i64,
    season_end_year: i16,
    db: &C,
) -> Result<(Vec<team_update::Model>, HashMap<i64, contract::Model>)>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let maybe_deadline_model = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        season_end_year,
        DeadlineType::PreseasonKeeper,
        db,
    )
    .await?;
    let deadline_model = maybe_deadline_model.ok_or_else(|| eyre!(
        "Could not find keeper deadline record for league {} (season end year: {}). One should already exist for the league and season when processing the keeper deadline.",
        league_id,
        season_end_year
    ))?;

    let maybe_keeper_deadline_transaction = deadline_model
        .find_related(transaction::Entity)
        .one(db)
        .await?;
    let keeper_deadline_transaction_model = maybe_keeper_deadline_transaction.ok_or_else(||eyre!("Could not find transaction associated with keeper deadline. One should already exist for the league and season when processing the keeper deadline."))?;
    let keeper_team_updates = keeper_deadline_transaction_model
        .find_related(team_update::Entity)
        .all(db)
        .await?;

    let active_league_contracts =
        contract_queries::find_active_contracts_in_league(league_id, db).await?;
    let active_league_contracts_by_id: HashMap<i64, contract::Model> = active_league_contracts
        .into_iter()
        .map(|contract_model| (contract_model.id, contract_model))
        .collect();

    Ok((keeper_team_updates, active_league_contracts_by_id))
}
