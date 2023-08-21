use chrono::{NaiveDate, Utc};
use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait};
use std::fmt::Debug;
use tracing::instrument;

use crate::{
    auction_bid,
    contract::{self},
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    transaction,
};

use super::ContractUpdatePlayerData;

/// Creates & inserts a team update from a completed auction.
pub async fn insert_team_update_from_auction_won<C>(
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

    let data = TeamUpdateData::Assets(vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
        contract_id: signed_contract_model.id,
        player_name_at_time_of_trade: contract_update_player_data.player_name,
        player_team_abbr_at_time_of_trade: contract_update_player_data.real_team_abbr,
        player_team_name_at_time_of_trade: contract_update_player_data.real_team_name,
        update_type: ContractUpdateType::AddViaAuction,
    }])]);
    let deadline_model = auction_transaction_model.get_deadline(db).await?;
    let team_model = winning_auction_bid_model.get_team(db).await?;
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

/// Updates the given team_update (generated via veteran auction processing) to be finished, along with an optional effective date (defaults to `now()` otherwise).
#[instrument]
pub async fn update_team_update_for_preseason_veteran_auction<C>(
    team_update_model: &team_update::Model,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait + Debug,
{
    let mut update_team_update_date_and_status: team_update::ActiveModel =
        team_update_model.clone().into();
    update_team_update_date_and_status.status = ActiveValue::Set(TeamUpdateStatus::Done);
    update_team_update_date_and_status.effective_date =
        ActiveValue::Set(maybe_override_effective_date.unwrap_or_else(|| Utc::now().date_naive()));

    let updated_model = update_team_update_date_and_status.update(db).await?;
    Ok(updated_model)
}
