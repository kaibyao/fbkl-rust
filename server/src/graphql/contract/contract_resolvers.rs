//! Contract reads. Read-only by design: contracts only ever change as a side effect of
//! trade/auction/keeper/roster operations, so there is no raw contract-edit mutation.

use async_graphql::{Context, Object, Result};
use fbkl_entity::{
    contract_queries::{find_contract_by_id, find_contract_chain},
    sea_orm::DatabaseConnection,
};

use super::Contract;
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

#[derive(Default)]
pub struct ContractQuery;

#[Object]
impl ContractQuery {
    /// A single contract, scoped to the caller's selected league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn contract(&self, ctx: &Context<'_>, id: i64) -> Result<Contract> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let model = find_contract_by_id(id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        if model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }

        Contract::from_model(&model).map_err(|_| code_error(ErrorCode::Internal))
    }

    /// The full history chain the given contract belongs to, oldest first.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn contract_chain(&self, ctx: &Context<'_>, contract_id: i64) -> Result<Vec<Contract>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let models = find_contract_chain(contract_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        if models
            .iter()
            .any(|model| model.league_id != caller_team.league_id)
        {
            return Err(code_error(ErrorCode::NotFound));
        }

        models
            .iter()
            .map(|model| Contract::from_model(model).map_err(|_| code_error(ErrorCode::Internal)))
            .collect()
    }
}
