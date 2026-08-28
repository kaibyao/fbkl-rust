use std::collections::HashSet;

use color_eyre::eyre::{Result, eyre};
use fbkl_entity::{
    contract::{self, ContractStatus},
    contract_queries, deadline,
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::{self, ContractUpdateType, TeamUpdateAsset, TeamUpdateData},
    team_update_queries,
    transaction::{self, TransactionKind},
    transaction_queries,
};
use tracing::instrument;

use crate::{
    deadline_processing::roster_lock::validate_team_roster,
    roster::{RosterMoveRejection, SalarySnapshot, calculate_team_contract_salary_with_model},
};

use super::drop_contract_team_update::create_drop_contract_team_update;

#[instrument(skip(db))]
pub async fn drop_contract_from_team<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    validate_contract_eligibility(&contract_model)?;
    validate_not_dropping_same_week_add(&contract_model, deadline_model, db).await?;

    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for a contract intended to be dropped (id = {})",
            contract_model.id
        )
    })?;
    let SalarySnapshot {
        salary: original_salary,
        cap: original_salary_cap,
    } = calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;

    // Saving the contract id for the transaction's contract_id, because the dropped one does not have a team_id and it becomes hard to calculate salary cap penalties without it.
    let contract_id = contract_model.id;

    let keeper_timing = if deadline_model.is_preseason_keeper_or_before() {
        contract_queries::PreseasonKeeperTiming::Before
    } else {
        contract_queries::PreseasonKeeperTiming::OnOrAfter
    };
    let dropped_contract =
        contract_queries::drop_contract(contract_model, keeper_timing, db).await?;

    // create transaction
    let transaction_to_insert = transaction::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(dropped_contract.end_of_season_year),
        kind: ActiveValue::Set(TransactionKind::TeamUpdateDropContract),
        league_id: ActiveValue::Set(dropped_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        contract_id: ActiveValue::Set(Some(contract_id)),
        ..Default::default()
    };
    let transaction_model =
        transaction_queries::insert_transaction(transaction_to_insert, db).await?;

    // create team_update
    create_drop_contract_team_update(
        &dropped_contract,
        deadline_model,
        &team_model,
        (original_salary, original_salary_cap),
        transaction_model.id,
        db,
    )
    .await?;

    Ok(dropped_contract)
}

fn validate_contract_eligibility(contract_model: &contract::Model) -> Result<()> {
    if contract_model.status == ContractStatus::Active {
        Ok(())
    } else {
        Err(RosterMoveRejection::ContractNotActive {
            contract_id: contract_model.id,
            status: contract_model.status,
        }
        .into())
    }
}

/// Rejects dropping a contract the team added this week before that week's adds sit legally on the
/// roster (rules §8.3.5 and §8.3.7).
///
/// The week is the set of `team_updates` whose transaction points at `deadline_model`, i.e. the
/// moves not yet locked in. A contract counts as a same-week add when every one of its updates
/// this week is an auction or trade add. Because the adds are already applied and the drop is not,
/// the team's current roster is the roster §8.3.7 asks about: with every add, without the drop.
#[instrument(skip(db))]
async fn validate_not_dropping_same_week_add<C>(
    contract_model: &contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let Some(team_id) = contract_model.team_id else {
        return Ok(());
    };
    if deadline_model.is_preseason_keeper_or_before() {
        return Ok(());
    }

    let this_week_team_updates =
        team_update_queries::find_team_updates_by_team(team_id, None, Some(deadline_model.id), db)
            .await?;
    let target_chain_contract_ids: HashSet<i64> =
        contract_queries::find_contract_chain(contract_model.id, db)
            .await?
            .into_iter()
            .map(|chain_contract| chain_contract.id)
            .collect();

    if !is_added_this_week(&this_week_team_updates, &target_chain_contract_ids)? {
        return Ok(());
    }

    // Rules 8.3.5/8.3.7: an added player must sit legally on the roster before he can be dropped, so the drop is illegal only while the roster carrying every add is still illegal.
    let violations = validate_team_roster(team_id, deadline_model, db).await?;
    if !violations.is_empty() {
        return Err(RosterMoveRejection::DropSameWeekAdd {
            contract_id: contract_model.id,
            violations: violations
                .iter()
                .map(|violation| violation.message.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        }
        .into());
    }

    Ok(())
}

/// Whether this week's moves added the target contract to the roster, i.e. whether rule 8.3.7's
/// "legally added before being dropped" clause applies to dropping it.
fn is_added_this_week(
    team_update_models: &[team_update::Model],
    target_chain_contract_ids: &HashSet<i64>,
) -> Result<bool> {
    let mut target_update_types = Vec::new();

    for team_update_model in team_update_models {
        let TeamUpdateData::Assets(asset_summary) = team_update_model.get_data()? else {
            continue;
        };
        for changed_asset in &asset_summary.changed_assets {
            let TeamUpdateAsset::Contracts(contract_updates) = changed_asset else {
                continue;
            };
            for contract_update in contract_updates {
                if target_chain_contract_ids.contains(&contract_update.contract_id) {
                    target_update_types.push(contract_update.update_type);
                }
            }
        }
    }

    Ok(!target_update_types.is_empty() && target_update_types.iter().copied().all(is_add))
}

/// Whether the update type is a weekly pickup, i.e. an add rule 8.3.7 asks to accommodate first.
///
/// Narrower than `is_add_from_outside_the_roster` in `move_contract_to_ir`: a rookie-draft add is
/// left out because the rookie draft lets an owner drop a just-drafted player, and RD contracts
/// take up no roster or cap space anyway.
const fn is_add(update_type: ContractUpdateType) -> bool {
    matches!(
        update_type,
        ContractUpdateType::AddViaAuction | ContractUpdateType::AddViaTrade
    )
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use fbkl_entity::team_update::{ContractUpdate, TeamUpdateStatus};

    use super::*;

    fn team_update_with_contract_update(
        id: i64,
        contract_id: i64,
        update_type: ContractUpdateType,
    ) -> team_update::Model {
        let data = TeamUpdateData::from_assets(
            vec![contract_id],
            vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
                contract_id,
                update_type,
                player_name_at_time: "Test Player".to_string(),
                player_team_abbr_at_time: "TST".to_string(),
                player_team_name_at_time: "Test Team".to_string(),
            }])],
            10,
            100,
            5,
            100,
        );
        let now = Utc::now().into();

        team_update::Model {
            id,
            data: data.to_json().unwrap(),
            effective_date: NaiveDate::from_ymd_opt(2024, 11, 4).unwrap(),
            sequence: None,
            status: TeamUpdateStatus::Pending,
            team_id: 1,
            transaction_id: Some(id),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn a_contract_added_this_week_is_recognised() {
        let team_updates = vec![
            team_update_with_contract_update(1, 100, ContractUpdateType::AddViaAuction),
            team_update_with_contract_update(2, 200, ContractUpdateType::AddViaTrade),
        ];
        let target_chain_contract_ids = HashSet::from([100]);

        assert!(is_added_this_week(&team_updates, &target_chain_contract_ids).unwrap());
    }

    #[test]
    fn a_contract_added_in_a_prior_week_is_not_added_this_week() {
        let team_updates = vec![team_update_with_contract_update(
            1,
            200,
            ContractUpdateType::AddViaAuction,
        )];
        let target_chain_contract_ids = HashSet::from([100]);

        assert!(!is_added_this_week(&team_updates, &target_chain_contract_ids).unwrap());
    }

    /// Rule 8.3.5 holds a lone add to the same "legally accommodated first" test, so being the
    /// week's only add does not exempt it; roster legality decides whether the drop goes through.
    #[test]
    fn the_weeks_only_add_still_counts_as_added_this_week() {
        let team_updates = vec![
            team_update_with_contract_update(1, 100, ContractUpdateType::AddViaAuction),
            team_update_with_contract_update(2, 200, ContractUpdateType::ToIR),
        ];
        let target_chain_contract_ids = HashSet::from([100]);

        assert!(is_added_this_week(&team_updates, &target_chain_contract_ids).unwrap());
    }

    #[test]
    fn a_contract_moved_but_not_added_this_week_is_not_added_this_week() {
        let team_updates = vec![team_update_with_contract_update(
            1,
            100,
            ContractUpdateType::ToIR,
        )];
        let target_chain_contract_ids = HashSet::from([100]);

        assert!(!is_added_this_week(&team_updates, &target_chain_contract_ids).unwrap());
    }
}
