use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use fbkl_entity::{
    contract, contract_queries,
    deadline::DeadlineType,
    deadline_queries,
    sea_orm::{ColumnTrait, ConnectionTrait, ModelTrait, QueryFilter},
    team_update::{self, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus},
    team_update_queries,
    transaction::{self, TransactionType},
};
use std::{collections::HashMap, fmt::Debug};
use tracing::instrument;

/// Processes the team_updates that have been created for the Keeper Deadline and sets the status for them.
#[instrument]
pub async fn process_keeper_deadline_transaction<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let (keeper_team_updates, mut active_league_contracts_by_id) =
        validate_and_get_process_keeper_deadline_data(league_id, end_of_season_year, db).await?;

    for team_update_model in keeper_team_updates {
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
            Ok(()) => {
                team_update_queries::update_team_update_status(
                    status_set_to_in_progress,
                    TeamUpdateStatus::Done,
                    db,
                )
                .await?;
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

    Ok(())
}

/// Processes the team_updates that have been created for the Keeper Deadline. Returns a tuple containing the number of contracts kept and dropped.
#[instrument]
async fn process_keeper_deadline_transaction_inner<C>(
    team_update_model: team_update::Model,
    active_league_contracts_by_id: &mut HashMap<i64, contract::Model>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let team_update_data = team_update_model.get_data()?;
    match team_update_data {
        TeamUpdateData::Assets(team_update_asset_summary) => {
            for updated_asset in team_update_asset_summary.changed_assets {
                match updated_asset {
                    TeamUpdateAsset::Contracts(updated_contracts) => {
                        for contract_update in updated_contracts {
                            match contract_update.update_type {
                                ContractUpdateType::Keeper => (),
                                ContractUpdateType::Drop => {
                                    let related_league_contract = active_league_contracts_by_id.remove(&contract_update.contract_id).ok_or_else(|| eyre!("Contract referred by keeper deadline team update is not an active contract or wasn't found. contract_id: {}", contract_update.contract_id))?;
                                    contract_queries::drop_contract(related_league_contract, true, db).await?;
                                },
                                x => bail!("Did not expect contract update type while processing keeper deadline: {:#?}", x),
                            }
                        }
                    },
                    TeamUpdateAsset::DraftPicks(updated_draft_picks) => bail!("Expected Keeper Deadline team update to be a contract update, but got draft pick instead: {:#?}", updated_draft_picks),
                }
            }
        }
        TeamUpdateData::Settings(_) => bail!("Expected Keeper Deadline team update to be a contract update, but got settings instead: {:#?}", team_update_data),
    };

    Ok(())
}

#[instrument]
async fn validate_and_get_process_keeper_deadline_data<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<(Vec<team_update::Model>, HashMap<i64, contract::Model>)>
where
    C: ConnectionTrait + Debug,
{
    let deadline_model = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineType::PreseasonKeeper,
        db,
    )
    .await?;

    let maybe_keeper_deadline_transaction = deadline_model
        .find_related(transaction::Entity)
        .filter(transaction::Column::TransactionType.eq(TransactionType::PreseasonKeeper))
        .one(db)
        .await?;
    let keeper_deadline_transaction_model = maybe_keeper_deadline_transaction.ok_or_else(||eyre!("Could not find transaction associated with keeper deadline. One should already exist for the league and season when processing the keeper deadline."))?;
    let keeper_team_updates = keeper_deadline_transaction_model
        .find_related(team_update::Entity)
        .filter(
            team_update::Column::Status
                .is_in(vec![TeamUpdateStatus::Pending, TeamUpdateStatus::Error]),
        )
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
