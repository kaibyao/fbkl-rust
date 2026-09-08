use color_eyre::eyre::{Result, eyre};
use fbkl_entity::{
    contract::{self, ContractKind, ContractStatus},
    contract_queries, deadline,
    league_event::{self, LeagueEventKind},
    league_event_queries,
    sea_orm::{ActiveValue, ConnectionTrait},
};
use tracing::instrument;

use crate::roster::{
    RosterMoveRejection, SalarySnapshot, calculate_team_contract_salary_with_model,
};

use super::rookie_activation_team_update::create_rookie_activation_team_update;

#[instrument(skip(db))]
pub async fn activate_rookie_development_contract<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    validate_contract_is_activatable(&contract_model)?;
    if !contract_model.is_latest_in_chain(db).await? {
        return Err(RosterMoveRejection::NotLatestInChain {
            contract_id: contract_model.id,
        }
        .into());
    }

    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for a RD(I) contract with id: {}",
            contract_model.id
        )
    })?;
    let SalarySnapshot {
        salary: original_salary,
        cap: original_salary_cap,
    } = calculate_team_contract_salary_with_model(&team_model, deadline_model, db).await?;
    let activated_contract =
        contract_queries::activate_rookie_development_contract(contract_model, db).await?;

    // create league event
    let league_event_to_insert = league_event::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(activated_contract.end_of_season_year),
        kind: ActiveValue::Set(LeagueEventKind::RookieContractActivation),
        league_id: ActiveValue::Set(activated_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        contract_id: ActiveValue::Set(Some(activated_contract.id)),
        ..Default::default()
    };
    let inserted_league_event =
        league_event_queries::insert_league_event(league_event_to_insert, db).await?;

    // create team_update
    create_rookie_activation_team_update(
        &activated_contract,
        deadline_model,
        &team_model,
        (original_salary, original_salary_cap),
        inserted_league_event.id,
        db,
    )
    .await?;

    Ok(activated_contract)
}

/// Guards activation against non-RD(I) or stale contracts.
fn validate_contract_is_activatable(contract_model: &contract::Model) -> Result<()> {
    if !matches!(
        contract_model.kind,
        ContractKind::RookieDevelopment | ContractKind::RookieDevelopmentInternational
    ) {
        return Err(RosterMoveRejection::NotRookieDevelopment {
            contract_id: contract_model.id,
            kind: contract_model.kind,
        }
        .into());
    }
    if contract_model.status != ContractStatus::Active {
        return Err(RosterMoveRejection::ContractNotActive {
            contract_id: contract_model.id,
            status: contract_model.status,
        }
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use fbkl_entity::contract::{ContractKind, ContractStatus, Model};

    use super::validate_contract_is_activatable;

    fn contract(kind: ContractKind, status: ContractStatus) -> Model {
        Model {
            id: 1,
            year_number: 1,
            kind,
            is_ir: false,
            salary: 10,
            end_of_season_year: 2025,
            status,
            league_id: 1,
            league_player_id: None,
            player_id: Some(1),
            previous_contract_id: None,
            original_contract_id: Some(1),
            team_id: Some(7),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn non_rookie_development_kind_is_rejected() {
        let contract_model = contract(ContractKind::Veteran, ContractStatus::Active);

        assert!(validate_contract_is_activatable(&contract_model).is_err());
    }

    #[test]
    fn replaced_contract_is_rejected() {
        let contract_model = contract(ContractKind::RookieDevelopment, ContractStatus::Replaced);

        assert!(validate_contract_is_activatable(&contract_model).is_err());
    }

    #[test]
    fn active_rd_and_rdi_are_accepted() {
        for kind in [
            ContractKind::RookieDevelopment,
            ContractKind::RookieDevelopmentInternational,
        ] {
            assert!(
                validate_contract_is_activatable(&contract(kind, ContractStatus::Active)).is_ok()
            );
        }
    }
}
