//! The league's read-only audit feed. Transactions are only ever written as a side effect of the
//! trade / auction / draft / keeper / roster engines, so this module exposes no mutations.

use async_graphql::{Context, Object, Result};
use fbkl_entity::{
    sea_orm::DatabaseConnection,
    transaction_queries::{find_transaction_by_id, find_transactions_in_league},
};

use super::{PagedTransactions, Transaction, TransactionFilter};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, require_league_role,
};

/// The feed spans a league's whole history (2014-15 onwards), so a page is always bounded.
const MAX_PAGE_SIZE: u64 = 100;

#[derive(Default)]
pub struct TransactionQuery;

#[Object]
impl TransactionQuery {
    /// One page of the caller's league's transaction history, newest first.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn transactions(
        &self,
        ctx: &Context<'_>,
        filter: Option<TransactionFilter>,
        #[graphql(default = 0)] page: u64,
        #[graphql(default = 25)] page_size: u64,
    ) -> Result<PagedTransactions> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let (maybe_team_id, maybe_kind) = filter.map_or((None, None), |f| (f.team_id, f.kind));

        let paged = find_transactions_in_league(
            caller_team.league_id,
            maybe_team_id,
            maybe_kind,
            page,
            page_size.min(MAX_PAGE_SIZE),
            db,
        )
        .await
        .map_err(|db_err| {
            tracing::error!(error = ?db_err, league_id = caller_team.league_id, "failed to load transactions");
            code_error(ErrorCode::Internal)
        })?;

        Ok(PagedTransactions {
            items: paged.items.iter().map(Transaction::from_model).collect(),
            total_items: paged.total_items,
        })
    }

    /// A single transaction, scoped to the caller's selected league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn transaction(&self, ctx: &Context<'_>, id: i64) -> Result<Transaction> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let model = find_transaction_by_id(id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        if model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }

        Ok(Transaction::from_model(&model))
    }
}
