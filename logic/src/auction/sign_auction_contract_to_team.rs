use chrono::NaiveDate;
use color_eyre::Result;
use fbkl_entity::{
    auction, auction_bid, auction_queries,
    contract::{self, ContractKind, FreeAgentException},
    contract_queries, deadline, league_event, league_event_queries, rfa_resolution_queries,
    sea_orm::{ActiveValue, ConnectionTrait, TransactionTrait},
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries::{self, ContractUpdatePlayerData},
};
use tracing::instrument;

use crate::roster::{
    SalarySnapshot, calculate_team_contract_salary, calculate_team_contract_salary_with_model,
};

/// Signs a contract to the team that submitted the last/winning bid to a preseason veteran auction before it ended. Creates + inserts the contract, league event, and team update.
///
/// `maybe_raised_bid_amount` sets the price for an RFA whose winner raised its own bid
/// (rules §15.3.2.1); every other signing pays the winning bid.
#[instrument(skip(db))]
pub async fn sign_auction_contract_to_team<C>(
    auction_model: &auction::Model,
    winning_auction_bid_model: &auction_bid::Model,
    deadline_model: &deadline::Model,
    maybe_raised_bid_amount: Option<i16>,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<(contract::Model, league_event::Model, team_update::Model)>
where
    C: ConnectionTrait + TransactionTrait,
{
    // Sign contract to team
    let winning_team_model = winning_auction_bid_model.get_team(db).await?;
    let SalarySnapshot {
        salary: previous_salary,
        cap: previous_salary_cap,
    } = calculate_team_contract_salary_with_model(&winning_team_model, deadline_model, db).await?;
    let auction_contract_model = auction_model.get_contract(db).await?;
    let fa_exception =
        find_free_agent_exception(&auction_contract_model, winning_team_model.id, db).await?;
    let signed_contract_model = contract_queries::sign_auction_contract_to_team(
        auction_model,
        maybe_raised_bid_amount.unwrap_or(winning_auction_bid_model.bid_amount),
        winning_team_model.id,
        fa_exception,
        db,
    )
    .await?;

    // Create league event
    let auction_league_event_model =
        league_event_queries::insert_auction_league_event(deadline_model, auction_model.id, db)
            .await?;

    // Create team_update
    let team_update_model = insert_team_update_from_auction_won(
        winning_auction_bid_model,
        &auction_league_event_model,
        &signed_contract_model,
        previous_salary,
        previous_salary_cap,
        maybe_override_effective_date,
        db,
    )
    .await?;

    Ok((
        signed_contract_model,
        auction_league_event_model,
        team_update_model,
    ))
}

/// Signs a recorded auction win to the team that won it, completing the auction (rules §8.3.6).
///
/// An in-season free agent auction closes to [`auction::AuctionStatus::Won`] without a contract, so
/// this is what turns a win into one: the owner's pickup calls it with the drops that make room,
/// and the roster lock calls it for any win nobody picked up. Pairs the signing with the status
/// change so a `Won` row cannot be signed twice.
#[instrument(skip(db))]
pub async fn sign_won_auction<C>(
    auction_model: &auction::Model,
    winning_bid_model: &auction_bid::Model,
    deadline_model: &deadline::Model,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<(contract::Model, team_update::Model)>
where
    C: ConnectionTrait + TransactionTrait,
{
    let (signed_contract_model, _, team_update_model) = sign_auction_contract_to_team(
        auction_model,
        winning_bid_model,
        deadline_model,
        None,
        maybe_override_effective_date,
        db,
    )
    .await?;
    let team_update_model = team_update_queries::update_team_update_for_auction(
        &team_update_model,
        maybe_override_effective_date,
        db,
    )
    .await?;
    auction_queries::update_auction_status(auction_model.id, auction::AuctionStatus::Completed, db)
        .await?;

    Ok((signed_contract_model, team_update_model))
}

/// Whether the auction winner also holds the player's re-sign discount (rules §15.4.2, §16.4.1).
///
/// The discount belongs to whoever owned the player at the keeper deadline, so a trade during the
/// auction hands over the player without handing over the discount. An RFA keeps that owner in its
/// resolution row; a UFA has no resolution, so its designated contract is the only record of it and
/// a UFA traded mid-auction still reads as its current team.
#[instrument(skip(db))]
async fn find_free_agent_exception<C>(
    auction_contract_model: &contract::Model,
    winning_team_id: i64,
    db: &C,
) -> Result<FreeAgentException>
where
    C: ConnectionTrait,
{
    let exception_holder_team_id = match auction_contract_model.kind {
        ContractKind::RestrictedFreeAgent => {
            rfa_resolution_queries::find_rfa_resolution_for_contract(auction_contract_model.id, db)
                .await?
                .map(|rfa_resolution_model| rfa_resolution_model.original_owner_team_id)
        }
        ContractKind::UnrestrictedFreeAgentOriginalTeam
        | ContractKind::UnrestrictedFreeAgentVeteran => auction_contract_model.team_id,
        _ => None,
    };

    Ok(if exception_holder_team_id == Some(winning_team_id) {
        FreeAgentException::Held
    } else {
        FreeAgentException::NotHeld
    })
}

/// Creates & inserts a team update from a completed auction.
#[instrument(skip(db))]
async fn insert_team_update_from_auction_won<C>(
    winning_auction_bid_model: &auction_bid::Model,
    auction_league_event_model: &league_event::Model,
    signed_contract_model: &contract::Model,
    previous_salary: i16,
    previous_salary_cap: i16,
    maybe_override_effective_date: Option<NaiveDate>,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait,
{
    let contract_update_player_data =
        ContractUpdatePlayerData::from_contract_model(signed_contract_model, db).await?;
    let deadline_model = auction_league_event_model.get_deadline(db).await?;
    let team_model = winning_auction_bid_model.get_team(db).await?;
    let current_active_team_contracts = team_model.get_active_contracts(db).await?;
    let SalarySnapshot {
        salary: new_salary,
        cap: new_salary_cap,
    } = calculate_team_contract_salary(
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

    let data = TeamUpdateData::from_assets(
        team_contract_ids,
        vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
            contract_id: signed_contract_model.id,
            player_name_at_time: contract_update_player_data.player_name,
            player_team_abbr_at_time: contract_update_player_data.real_team_abbr,
            player_team_name_at_time: contract_update_player_data.real_team_name,
            update_type: ContractUpdateType::AddViaAuction,
        }])],
        new_salary,
        new_salary_cap,
        previous_salary,
        previous_salary_cap,
    );

    let new_team_update = team_update::ActiveModel {
        id: ActiveValue::NotSet,
        data: ActiveValue::Set(data.to_json()?),
        effective_date: ActiveValue::Set(
            maybe_override_effective_date.unwrap_or_else(|| deadline_model.date_time.date_naive()),
        ),
        transaction_number: ActiveValue::NotSet,
        status: ActiveValue::Set(TeamUpdateStatus::Pending),
        team_id: ActiveValue::Set(team_model.id),
        league_event_id: ActiveValue::Set(Some(auction_league_event_model.id)),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    let inserted_team_update = team_update_queries::insert_team_update(new_team_update, db).await?;
    Ok(inserted_team_update)
}
