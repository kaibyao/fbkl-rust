//! The league event + `team_update` rows every RFA handshake step writes (rules §15.3).
//!
//! All of them date themselves from `PreseasonFaAuctionStart`, the same deadline the veteran
//! auction's own signings use: the handshake is the tail of that auction, and the league has no
//! separate deadline for it.

use color_eyre::Result;
use fbkl_entity::{
    contract, contract_queries, deadline,
    deadline::DeadlineKind,
    deadline_queries, draft_pick,
    league_event::{self, LeagueEventKind},
    league_event_queries, rfa_resolution,
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::{
        self, ContractUpdate, ContractUpdateType, DraftPickUpdate, DraftPickUpdateType,
        TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries::{self, ContractUpdatePlayerData},
};
use tracing::instrument;

use crate::roster::{SalarySnapshot, calculate_team_contract_salary};

/// The deadline every RFA handshake league event is tied to.
#[instrument(skip(db))]
pub(super) async fn find_rfa_handshake_deadline<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    db: &C,
) -> Result<deadline::Model>
where
    C: ConnectionTrait,
{
    deadline_queries::find_deadline_for_season_by_type(
        rfa_resolution_model.league_id,
        rfa_resolution_model.end_of_season_year,
        DeadlineKind::PreseasonFaAuctionStart,
        db,
    )
    .await
}

/// Records one handshake step. `maybe_contract_id` names the contract the step acted on, and is
/// NULL for a raise, which changes no contract.
#[instrument(skip(db))]
pub(super) async fn insert_rfa_league_event<C>(
    rfa_resolution_model: &rfa_resolution::Model,
    kind: LeagueEventKind,
    maybe_contract_id: Option<i64>,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<league_event::Model>
where
    C: ConnectionTrait,
{
    let league_event_to_insert = league_event::ActiveModel {
        end_of_season_year: ActiveValue::Set(rfa_resolution_model.end_of_season_year),
        kind: ActiveValue::Set(kind),
        league_id: ActiveValue::Set(rfa_resolution_model.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        contract_id: ActiveValue::Set(maybe_contract_id),
        ..Default::default()
    };
    league_event_queries::insert_league_event(league_event_to_insert, db).await
}

/// The roster history entry for the original owner re-signing the player (rules §15.3.2).
#[instrument(skip(db))]
pub(super) async fn insert_rfa_resign_team_update<C>(
    signed_contract_model: &contract::Model,
    deadline_model: &deadline::Model,
    (previous_salary, previous_salary_cap): (i16, i16),
    league_event_id: i64,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait,
{
    let signing_team_id = signed_contract_model.team_id.unwrap_or_default();
    let team_active_contracts =
        contract_queries::find_active_contracts_for_team(signing_team_id, db).await?;
    let related_player_data =
        ContractUpdatePlayerData::from_contract_model(signed_contract_model, db).await?;
    let SalarySnapshot {
        salary: new_salary,
        cap: new_salary_cap,
    } = calculate_team_contract_salary(signing_team_id, &team_active_contracts, deadline_model, db)
        .await?;

    let team_update_data = TeamUpdateData::from_assets(
        team_active_contracts
            .iter()
            .map(|team_contract| team_contract.id)
            .collect(),
        vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
            contract_id: signed_contract_model.id,
            update_type: ContractUpdateType::RfaResign,
            player_name_at_time: related_player_data.player_name,
            player_team_abbr_at_time: related_player_data.real_team_abbr,
            player_team_name_at_time: related_player_data.real_team_name,
        }])],
        new_salary,
        new_salary_cap,
        previous_salary,
        previous_salary_cap,
    );

    insert_team_update(
        team_update_data,
        signing_team_id,
        deadline_model,
        league_event_id,
        db,
    )
    .await
}

/// The roster history entry for one side of a compensation pick changing hands (rules §15.2).
/// Salaries are unchanged by a pick move, so the same snapshot serves as before and after.
#[instrument(skip(db))]
pub(super) async fn insert_compensation_pick_team_update<C>(
    team_id: i64,
    forfeited_draft_pick_model: &draft_pick::Model,
    update_type: DraftPickUpdateType,
    deadline_model: &deadline::Model,
    league_event_id: i64,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait,
{
    let team_active_contracts =
        contract_queries::find_active_contracts_for_team(team_id, db).await?;
    let SalarySnapshot { salary, cap } =
        calculate_team_contract_salary(team_id, &team_active_contracts, deadline_model, db).await?;

    let team_update_data = TeamUpdateData::from_assets(
        team_active_contracts
            .iter()
            .map(|team_contract| team_contract.id)
            .collect(),
        vec![TeamUpdateAsset::DraftPicks(vec![DraftPickUpdate {
            draft_pick_id: forfeited_draft_pick_model.id,
            update_type,
            added_draft_pick_option_id: None,
        }])],
        salary,
        cap,
        salary,
        cap,
    );

    insert_team_update(
        team_update_data,
        team_id,
        deadline_model,
        league_event_id,
        db,
    )
    .await
}

/// Every handshake `team_update` takes effect the moment it is written, so it goes in `Done`.
async fn insert_team_update<C>(
    team_update_data: TeamUpdateData,
    team_id: i64,
    deadline_model: &deadline::Model,
    league_event_id: i64,
    db: &C,
) -> Result<team_update::Model>
where
    C: ConnectionTrait,
{
    let team_update_to_insert = team_update::ActiveModel {
        data: ActiveValue::Set(team_update_data.to_json()?),
        effective_date: ActiveValue::Set(deadline_model.date_time.date_naive()),
        status: ActiveValue::Set(TeamUpdateStatus::Done),
        team_id: ActiveValue::Set(team_id),
        league_event_id: ActiveValue::Set(Some(league_event_id)),
        ..Default::default()
    };
    team_update_queries::insert_team_update(team_update_to_insert, db).await
}
