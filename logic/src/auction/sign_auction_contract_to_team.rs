use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    auction, auction_bid, contract, contract_queries, deadline,
    sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionTrait},
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateAssetSummary,
        TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries::ContractUpdatePlayerData,
    transaction, transaction_queries,
};
use tracing::instrument;

use crate::roster::calculate_team_contract_salary;

/// Signs a contract to the team that submitted the last/winning bid to an auction before it ended. Creates + inserts the contract, transaction, and team update.
#[instrument]
pub async fn sign_auction_contract_to_team<C>(
    auction_model: &auction::Model,
    winning_auction_bid_model: &auction_bid::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<(contract::Model, transaction::Model, team_update::Model)>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    // Sign contract to team
    let signed_contract_model = contract_queries::sign_auction_contract_to_team(
        auction_model,
        winning_auction_bid_model,
        db,
    )
    .await?;

    // Create transaction
    let auction_transaction_model =
        transaction_queries::insert_auction_transaction(deadline_model, auction_model.id, db)
            .await?;

    // Create team_update
    let team_update_model = insert_team_update_from_auction_won(
        winning_auction_bid_model,
        &auction_transaction_model,
        &signed_contract_model,
        db,
    )
    .await?;

    Ok((
        signed_contract_model,
        auction_transaction_model,
        team_update_model,
    ))
}

/// Creates & inserts a team update from a completed auction.
#[instrument]
async fn insert_team_update_from_auction_won<C>(
    winning_auction_bid_model: &auction_bid::Model,
    auction_transaction_model: &transaction::Model,
    signed_contract_model: &contract::Model,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let contract_update_player_data =
        ContractUpdatePlayerData::from_contract_model(signed_contract_model, db).await?;
    let deadline_model = auction_transaction_model.get_deadline(db).await?;
    let team_model = winning_auction_bid_model.get_team(db).await?;
    let current_active_team_contracts = team_model.get_active_contracts(db).await?;
    // start here. need to generate this team update object.
    let (previous_salary, previous_salary_cap) = calculate_team_contract_salary(
        team_model.id,
        &current_active_team_contracts,
        &deadline_model,
        db,
    )
    .await?;

    let mut team_contract_ids: Vec<i64> = current_active_team_contracts
        .iter()
        .map(|contract_model| contract_model.id)
        .collect();
    team_contract_ids.push(signed_contract_model.id);

    let data = TeamUpdateData::Assets(TeamUpdateAssetSummary {
        all_contract_ids: team_contract_ids,
        changed_assets: vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
            contract_id: signed_contract_model.id,
            player_name_at_time_of_trade: contract_update_player_data.player_name,
            player_team_abbr_at_time_of_trade: contract_update_player_data.real_team_abbr,
            player_team_name_at_time_of_trade: contract_update_player_data.real_team_name,
            update_type: ContractUpdateType::AddViaAuction,
        }])],
        new_salary: previous_salary + signed_contract_model.salary,
        new_salary_cap: previous_salary_cap,
        previous_salary,
        previous_salary_cap,
    });

    let new_team_update = team_update::ActiveModel {
        id: ActiveValue::NotSet,
        data: ActiveValue::Set(data.as_bytes()?),
        effective_date: ActiveValue::Set(deadline_model.date_time.date_naive()),
        status: ActiveValue::Set(TeamUpdateStatus::Pending),
        team_id: ActiveValue::Set(team_model.id),
        transaction_id: ActiveValue::Set(Some(auction_transaction_model.id)),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    let inserted_team_update = new_team_update.insert(db).await?;

    Ok(inserted_team_update)
}
