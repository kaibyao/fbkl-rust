use std::collections::HashSet;

use color_eyre::eyre::{Result, eyre};
use fbkl_entity::{
    contract, contract_queries,
    deadline::{self, DeadlineKind},
    league_event::{self, LeagueEventKind},
    league_event_queries,
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::{
        ContractUpdateType, TeamUpdateAsset, TeamUpdateAssetSummary, TeamUpdateData,
        TeamUpdateStatus,
    },
    team_update_queries,
};
use tracing::instrument;

use crate::roster::{
    RosterMoveRejection, SalarySnapshot, calculate_team_contract_salary_with_model,
};

use super::ir_team_update::create_ir_team_update;

#[instrument(skip(db))]
pub async fn move_contract_to_ir<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    validate_ir_eligible_in_season(&contract_model, deadline_model, db).await?;

    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for an contract with id: {}",
            contract_model.id
        )
    })?;
    let SalarySnapshot {
        salary: original_salary,
        cap: original_salary_cap,
    } = calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;

    let updated_contract = contract_queries::move_contract_to_ir(contract_model, db).await?;

    // create league event
    let ir_league_event_to_insert = league_event::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(updated_contract.end_of_season_year),
        kind: ActiveValue::Set(LeagueEventKind::TeamUpdateToIr),
        league_id: ActiveValue::Set(updated_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        contract_id: ActiveValue::Set(Some(updated_contract.id)),
        ..Default::default()
    };
    let ir_league_event =
        league_event_queries::insert_league_event(ir_league_event_to_insert, db).await?;

    // create team_update
    create_ir_team_update(
        &updated_contract,
        deadline_model,
        &team_model,
        ContractUpdateType::ToIR,
        (original_salary, original_salary_cap),
        ir_league_event.id,
        db,
    )
    .await?;

    Ok(updated_contract)
}

/// Rejects a move to IR that the league rules do not allow (rules §5.1.3, §10.1.2, §10.3.2, §11.7).
///
/// A contract may go straight to IR only at the preseason final roster lock. At every other
/// deadline a committed `team_update` must already show the contract on this team without IR, and
/// the add that brought it in only counts once its own week is over, so that an owner cannot park a
/// fresh signing or trade pickup on IR to dodge the 22-man limit (rules §10.3.1).
#[instrument(skip(db))]
async fn validate_ir_eligible_in_season<C>(
    contract_model: &contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    if contract_model.is_ir {
        return Err(RosterMoveRejection::AlreadyInIr {
            contract_id: contract_model.id,
        }
        .into());
    }

    if deadline_model.kind == DeadlineKind::PreseasonFinalRosterLock {
        return Ok(());
    }

    let team_id = contract_model.team_id.ok_or_else(|| {
        eyre!(
            "Cannot move a contract to IR when it is not on a team. (contract_id = {})",
            contract_model.id
        )
    })?;

    let non_ir_chain_contract_ids: HashSet<i64> =
        contract_queries::find_contract_chain(contract_model.id, db)
            .await?
            .into_iter()
            .filter(|chain_contract| !chain_contract.is_ir)
            .map(|chain_contract| chain_contract.id)
            .collect();

    let committed_team_updates = team_update_queries::find_team_updates_by_team(
        team_id,
        Some(TeamUpdateStatus::Done),
        None,
        db,
    )
    .await?;
    let this_week_update_ids: HashSet<i64> = team_update_queries::find_team_updates_by_team(
        team_id,
        Some(TeamUpdateStatus::Done),
        Some(deadline_model.id),
        db,
    )
    .await?
    .into_iter()
    .map(|team_update_model| team_update_model.id)
    .collect();

    let mut is_previously_committed_without_ir = false;
    for team_update_model in committed_team_updates {
        let TeamUpdateData::Assets(asset_summary) = team_update_model.get_data()? else {
            continue;
        };
        let is_on_committed_roster = asset_summary
            .all_contract_ids
            .iter()
            .any(|committed_contract_id| non_ir_chain_contract_ids.contains(committed_contract_id));
        let is_add_still_in_its_own_week = this_week_update_ids.contains(&team_update_model.id)
            && is_acquisition_of(&asset_summary, &non_ir_chain_contract_ids);

        if is_on_committed_roster && !is_add_still_in_its_own_week {
            is_previously_committed_without_ir = true;
            break;
        }
    }

    if !is_previously_committed_without_ir {
        return Err(RosterMoveRejection::StraightToIr {
            contract_id: contract_model.id,
            deadline_kind: deadline_model.kind,
        }
        .into());
    }

    Ok(())
}

/// Whether this `team_update` is the add that put one of `chain_contract_ids` on the team.
///
/// An in-season auction win or trade receipt flips its own `team_update` to Done as soon as it
/// happens, so that update proves nothing about the contract ever fitting on the 22-man active
/// roster while its week is still open. Once the week's lock has fired the add is filed under a
/// past deadline, and the roster it committed does count (rules §10.3.1).
// ponytail: an add under a past deadline is taken as proof its lock committed the roster. A lock
// whose job never ran would also pass; the fix would be a per-lock committed marker row.
fn is_acquisition_of(
    asset_summary: &TeamUpdateAssetSummary,
    chain_contract_ids: &HashSet<i64>,
) -> bool {
    asset_summary
        .changed_assets
        .iter()
        .any(|changed_asset| match changed_asset {
            TeamUpdateAsset::Contracts(contract_updates) => {
                contract_updates.iter().any(|contract_update| {
                    chain_contract_ids.contains(&contract_update.contract_id)
                        && is_add_from_outside_the_roster(contract_update.update_type)
                })
            }
            TeamUpdateAsset::DraftPicks(_) => false,
        })
}

/// Whether the update type brings a contract onto the team from outside its roster.
const fn is_add_from_outside_the_roster(update_type: ContractUpdateType) -> bool {
    matches!(
        update_type,
        ContractUpdateType::AddViaAuction
            | ContractUpdateType::AddViaTrade
            | ContractUpdateType::AddViaRookieDraft
    )
}
