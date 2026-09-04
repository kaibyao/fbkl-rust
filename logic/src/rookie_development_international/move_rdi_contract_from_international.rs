use color_eyre::eyre::{Result, eyre};
use fbkl_entity::{
    contract::{self, ContractKind},
    contract_queries, deadline,
    league_event::{self, LeagueEventKind},
    league_event_queries,
    sea_orm::{ActiveValue, ConnectionTrait},
    team_update::ContractUpdateType,
};
use tracing::instrument;

use super::{rdi_team_update::create_rdi_move_team_update, validate_contract_kind};

#[instrument(skip(db))]
pub async fn move_rookie_development_international_contract_to_stateside<C>(
    contract_model: contract::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<contract::Model>
where
    C: ConnectionTrait,
{
    // §11.3.1 forced transition: leaving international is always legal, so kind is the only gate.
    validate_contract_kind(
        &contract_model,
        ContractKind::RookieDevelopmentInternational,
    )?;

    let team_model = contract_model.get_team(db).await?.ok_or_else(|| {
        eyre!(
            "Could not retrieve the expected team for an RD contract with id: {}",
            contract_model.id
        )
    })?;
    let moved_contract = contract_queries::move_rdi_contract_to_rd(contract_model, db).await?;

    // create league event
    let league_event_to_insert = league_event::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(moved_contract.end_of_season_year),
        kind: ActiveValue::Set(LeagueEventKind::TeamUpdateFromRdi),
        league_id: ActiveValue::Set(moved_contract.league_id),
        deadline_id: ActiveValue::Set(deadline_model.id),
        contract_id: ActiveValue::Set(Some(moved_contract.id)),
        ..Default::default()
    };
    let inserted_league_event =
        league_event_queries::insert_league_event(league_event_to_insert, db).await?;

    // create team_update
    create_rdi_move_team_update(
        &moved_contract,
        deadline_model,
        &team_model,
        ContractUpdateType::FromRdi,
        inserted_league_event.id,
        db,
    )
    .await?;

    Ok(moved_contract)
}
