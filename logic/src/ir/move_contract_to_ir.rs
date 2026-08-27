use std::collections::HashSet;

use color_eyre::eyre::{Result, ensure, eyre};
use fbkl_entity::{
    contract, contract_queries,
    deadline::{self, DeadlineKind},
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::{ContractUpdateType, TeamUpdateData, TeamUpdateStatus},
    team_update_queries,
    transaction::{self, TransactionKind},
    transaction_queries,
};
use tracing::instrument;

use crate::roster::{SalarySnapshot, calculate_team_contract_salary_with_model};

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

    // create transaction
    let ir_transaction_to_insert = transaction::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(updated_contract.end_of_season_year),
        kind: ActiveValue::Set(TransactionKind::TeamUpdateToIr),
        league_id: ActiveValue::Set(updated_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        contract_id: ActiveValue::Set(Some(updated_contract.id)),
        ..Default::default()
    };
    let ir_transaction =
        transaction_queries::insert_transaction(ir_transaction_to_insert, db).await?;

    // create team_update
    create_ir_team_update(
        &updated_contract,
        deadline_model,
        &team_model,
        ContractUpdateType::ToIR,
        (original_salary, original_salary_cap),
        ir_transaction.id,
        db,
    )
    .await?;

    Ok(updated_contract)
}

/// Rejects a move to IR that the league rules do not allow (rules §5.1.3, §10.1.2, §10.3.2, §11.7).
///
/// A contract may go straight to IR only at the preseason final roster lock. At every other
/// deadline the contract must already have been committed to this team without IR at an earlier
/// lock, so that an owner cannot park a fresh signing on IR to dodge the 22-man limit.
#[instrument(skip(db))]
pub async fn validate_ir_eligible_in_season<C>(
    contract_model: &contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    ensure!(
        !contract_model.is_ir,
        "Cannot move a contract to IR when it is already in IR. (contract_id = {})",
        contract_model.id
    );

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

    let mut is_previously_committed_without_ir = false;
    for team_update_model in committed_team_updates {
        if let TeamUpdateData::Assets(asset_summary) = team_update_model.get_data()?
            && asset_summary
                .all_contract_ids
                .iter()
                .any(|committed_contract_id| {
                    non_ir_chain_contract_ids.contains(committed_contract_id)
                })
        {
            is_previously_committed_without_ir = true;
            break;
        }
    }

    ensure!(
        is_previously_committed_without_ir,
        "Cannot move a contract straight to IR outside of the preseason final roster lock. The contract must first be committed to the team without IR. (contract_id = {}, deadline_kind = {:?})",
        contract_model.id,
        deadline_model.kind
    );

    Ok(())
}
