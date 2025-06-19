use async_graphql::{Context, Object, Result, SimpleObject};
use fbkl_entity::{
    contract_queries::find_active_contracts_for_team,
    deadline_queries::find_most_recent_deadline_by_datetime,
    sea_orm::{prelude::DateTimeWithTimeZone, DatabaseConnection},
    team,
    team_user_queries::get_team_users_by_team,
};
use fbkl_logic::roster::calculate_team_contract_salary;

use crate::graphql::contract::Contract;

use super::TeamUser;

#[derive(Clone, Default)]
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

    async fn contracts(&self, ctx: &Context<'_>) -> Result<Vec<Contract>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let contract_models = find_active_contracts_for_team(self.id, db).await?;

        Ok(contract_models
            .into_iter()
            .map(Contract::from_model)
            .collect())
    }

    async fn salary_cap(&self, ctx: &Context<'_>, datetime_str: String) -> Result<TeamSalaryCap> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        // Parse ISO datetime string (RFC3339 format)
        let datetime = datetime_str
            .parse::<DateTimeWithTimeZone>()
            .map_err(|e| format!("Failed to parse datetime string '{}': {}", datetime_str, e))?;

        let deadline = find_most_recent_deadline_by_datetime(self.league_id, datetime, db).await?;
        let contract_models = find_active_contracts_for_team(self.id, db).await?;
        let (total_contract_amount, max_salary_cap_for_deadline) =
            calculate_team_contract_salary(self.id, &contract_models, &deadline, db).await?;

        let salary_cap = TeamSalaryCap {
            salary_cap: max_salary_cap_for_deadline,
            salary_used: total_contract_amount,
        };

        Ok(salary_cap)
    }

    async fn team_users(&self, ctx: &Context<'_>) -> Result<Vec<TeamUser>> {
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
