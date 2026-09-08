use async_graphql::{Context, Object, Result, SimpleObject};
use fbkl_entity::{
    contract_queries::find_active_contracts_for_team,
    sea_orm::{DatabaseConnection, prelude::DateTimeWithTimeZone},
    team,
    team_update::{self, TeamUpdateStatus},
    team_update_queries::find_team_updates_by_team,
    team_user::LeagueRole,
    team_user_queries::get_team_users_by_team,
};
use fbkl_logic::roster::calculate_team_contract_salary_at_datetime;

use crate::{
    error::FbklError,
    graphql::{ErrorCode, RoleRequirement, code_error, contract::Contract, require_league_role},
};

use super::TeamUser;

#[derive(Clone, Default)]
#[allow(clippy::struct_field_names)] // field names mirror GraphQL schema
pub struct Team {
    pub id: i64,
    pub name: String,
    pub league_id: i64,
    pub team_users: Vec<TeamUser>,
    // TODO: Eventually add draft picks
}

#[derive(SimpleObject)]
pub struct TeamSalaryCap {
    pub salary_cap: i16,
    pub salary_used: i16,
}

/// One recorded change to a team's roster or settings. `data` is the raw
/// `TeamUpdateData` json — typing its variants is deferred until a client needs it.
#[derive(SimpleObject)]
pub struct TeamUpdate {
    pub id: i64,
    pub team_id: i64,
    pub effective_date: String,
    /// Which transaction of the week this move belongs to; `None` means insertion order.
    pub transaction_number: Option<i16>,
    pub status: TeamUpdateStatus,
    pub league_event_id: Option<i64>,
    pub data: String,
}

impl TeamUpdate {
    pub fn from_model(entity: &team_update::Model) -> Self {
        Self {
            id: entity.id,
            team_id: entity.team_id,
            effective_date: entity.effective_date.to_string(),
            transaction_number: entity.transaction_number,
            status: entity.status,
            league_event_id: entity.league_event_id,
            data: entity.data.to_string(),
        }
    }
}

impl Team {
    pub fn from_model(entity: team::Model) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            league_id: entity.league_id,
            team_users: vec![],
        }
    }
}

#[Object]
impl Team {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn league_id(&self) -> i64 {
        self.league_id
    }

    async fn contracts(&self, ctx: &Context<'_>) -> Result<Vec<Contract>, FbklError> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let contract_models = find_active_contracts_for_team(self.id, db).await?;

        contract_models
            .iter()
            .map(Contract::from_model)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn salary_cap(
        &self,
        ctx: &Context<'_>,
        datetime_str: String,
    ) -> Result<TeamSalaryCap, FbklError> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let datetime = datetime_str.parse::<DateTimeWithTimeZone>().map_err(|e| {
            FbklError::BadRequest(format!(
                "Failed to parse datetime string '{datetime_str}': {e}"
            ))
        })?;

        let snapshot =
            calculate_team_contract_salary_at_datetime(self.league_id, self.id, datetime, db)
                .await?;

        let salary_cap = TeamSalaryCap {
            salary_cap: snapshot.cap,
            salary_used: snapshot.salary,
        };

        Ok(salary_cap)
    }

    /// A team's change history. Only the team's own members and the commissioner may read it.
    async fn team_updates(
        &self,
        ctx: &Context<'_>,
        status: Option<TeamUpdateStatus>,
    ) -> Result<Vec<TeamUpdate>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, _) = require_league_role(ctx, RoleRequirement::Member).await?;

        let is_commissioner = team_user.league_role == LeagueRole::LeagueCommissioner;
        if team_user.team_id != self.id && !is_commissioner {
            return Err(code_error(ErrorCode::Forbidden));
        }

        let team_update_models = find_team_updates_by_team(self.id, status, None, db)
            .await
            .map_err(|db_err| {
                tracing::error!(error = ?db_err, team_id = self.id, "failed to load team updates");
                code_error(ErrorCode::Internal)
            })?;

        Ok(team_update_models
            .iter()
            .map(TeamUpdate::from_model)
            .collect())
    }

    async fn team_users(&self, ctx: &Context<'_>) -> Result<Vec<TeamUser>, FbklError> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user_models = get_team_users_by_team(self.id, db).await?;

        let team_users = team_user_models
            .into_iter()
            .map(|team_user_model| TeamUser {
                league_role: team_user_model.league_role,
                nickname: team_user_model.nickname,
                team: None,
                team_id: team_user_model.team_id,
                user: None,
                user_id: team_user_model.user_id,
            })
            .collect();

        Ok(team_users)
    }
}
