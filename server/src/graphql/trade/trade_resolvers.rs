//! Trade proposal / acceptance / rejection.
//!
//! Cap and roster legality are deliberately *not* checked here — `logic::trade` validates asset
//! ownership only (see `logic/CLAUDE.md`); legality lands with fbkl-rust-8zs.

use async_graphql::{Context, Error as GraphQlError, Object, Result};
use chrono::Utc;
use color_eyre::Report;
use fbkl_entity::{
    contract::ContractStatus,
    contract_queries::find_contract_by_id,
    deadline_queries::{MissingSeasonDeadline, find_most_recent_deadline_by_datetime},
    sea_orm::DatabaseConnection,
    trade,
    trade_asset::{ToTeamId, TradeAssetType},
    trade_asset_queries::new_trade_asset_active_model_by_id,
    trade_queries::{find_active_trades_for_team, find_active_trades_in_league, find_trade_by_id},
};
use fbkl_logic::trade::{
    MissingPreTradeSalary, MissingUpcomingRosterLock, TradeLegality, accept_trade, propose_trade,
    reject_trade,
};

use super::{ProposeTradeInput, Trade};
use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, graphql_error, require_league_role,
    roster::roster_move_error,
};

#[derive(Default)]
pub struct TradeQuery;

#[Object]
impl TradeQuery {
    /// Trades the caller's own team can still act on.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn proposed_trades(&self, ctx: &Context<'_>) -> Result<Vec<Trade>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, _) = require_league_role(ctx, RoleRequirement::Member).await?;

        let trades = find_active_trades_for_team(team_user.team_id, db)
            .await
            .map_err(|err| internal("failed to load a team's trades", &err))?;

        Ok(trades.into_iter().map(Trade::from_model).collect())
    }

    /// Every still-actionable trade in the caller's league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn active_trades(&self, ctx: &Context<'_>) -> Result<Vec<Trade>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let trades = find_active_trades_in_league(caller_team.league_id, db)
            .await
            .map_err(|err| internal("failed to load league trades", &err))?;

        Ok(trades.into_iter().map(Trade::from_model).collect())
    }

    /// A single trade, scoped to the caller's selected league.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn trade(&self, ctx: &Context<'_>, id: i64) -> Result<Trade> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let model = find_trade_by_id(id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;

        if model.league_id != caller_team.league_id {
            return Err(code_error(ErrorCode::NotFound));
        }

        Ok(Trade::from_model(model))
    }
}

#[derive(Default)]
pub struct TradeMutation;

#[Object]
impl TradeMutation {
    /// Proposes a trade from the caller's own team to one or more other teams.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn propose_trade(&self, ctx: &Context<'_>, input: ProposeTradeInput) -> Result<Trade> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        if input.from_team_id != team_user.team_id {
            return Err(code_error(ErrorCode::Forbidden));
        }
        if input.to_teams.is_empty() {
            return Err(graphql_error(
                ErrorCode::BadRequest,
                "a trade needs at least one other team",
            ));
        }

        let deadline = find_most_recent_deadline_by_datetime(
            caller_team.league_id,
            Utc::now().fixed_offset(),
            db,
        )
        .await
        .map_err(|err| internal("failed to resolve the current season", &err))?;

        let mut to_team_ids: Vec<i64> = vec![];
        let mut trade_assets = vec![];
        for group in &input.to_teams {
            if group.to_team_id == input.from_team_id {
                return Err(graphql_error(
                    ErrorCode::BadRequest,
                    "a team cannot trade with itself",
                ));
            }
            if !to_team_ids.contains(&group.to_team_id) {
                to_team_ids.push(group.to_team_id);
            }

            for asset in &group.assets {
                let active_model = new_trade_asset_active_model_by_id(
                    asset.asset_type,
                    asset.asset_id,
                    ToTeamId(group.to_team_id),
                    db,
                )
                .await
                .map_err(|err| graphql_error(ErrorCode::BadRequest, err.to_string()))?;
                trade_assets.push(active_model);
            }
        }

        if trade_assets.is_empty() {
            return Err(graphql_error(
                ErrorCode::BadRequest,
                "a trade needs at least one asset",
            ));
        }

        // Reject assets owned by a team outside the trade — the DB-derived owner must be involved.
        let involved: Vec<i64> = std::iter::once(input.from_team_id)
            .chain(to_team_ids.iter().copied())
            .collect();
        for active_model in &trade_assets {
            if !involved.contains(active_model.from_team_id.as_ref()) {
                return Err(graphql_error(
                    ErrorCode::BadRequest,
                    "an asset belongs to a team that is not part of this trade",
                ));
            }
        }

        // A proposer receives nothing under this input shape, so every drop has to be theirs now.
        validate_accommodating_drops(
            &input.accommodating_drop_contract_ids,
            input.from_team_id,
            caller_team.league_id,
            &[],
            db,
        )
        .await?;

        let proposed = propose_trade(
            caller_team.league_id,
            deadline.end_of_season_year,
            &team_user,
            &to_team_ids,
            trade_assets,
            &input.accommodating_drop_contract_ids,
            db,
        )
        .await
        .map_err(|err| internal("failed to propose trade", &err))?;

        Ok(Trade::from_model(proposed))
    }

    /// Accepts a trade, with the drops that make it fit the accepting owner's roster. Once every
    /// involved team has accepted, the trade is processed immediately.
    ///
    /// The accept is the owner's one chance to submit those drops: a trade and one owner's
    /// accommodating drops are one transaction, judged together when the trade processes (rules
    /// §12.5.3, §13.1.4). A drop may name a contract this trade brings in, which T2 then refuses.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn accept_trade(
        &self,
        ctx: &Context<'_>,
        trade_id: i64,
        accommodating_drop_contract_ids: Vec<i64>,
    ) -> Result<Trade> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let model =
            load_actionable_trade(ctx, trade_id, team_user.team_id, caller_team.league_id).await?;

        let trade_assets = model
            .get_trade_assets(db)
            .await
            .map_err(|err| internal("failed to load trade assets", &err))?;
        let incoming_contract_ids: Vec<i64> = trade_assets
            .iter()
            .filter(|trade_asset_model| {
                trade_asset_model.asset_type == TradeAssetType::Contract
                    && trade_asset_model.to_team_id == team_user.team_id
            })
            .filter_map(|trade_asset_model| trade_asset_model.contract_id)
            .collect();
        validate_accommodating_drops(
            &accommodating_drop_contract_ids,
            team_user.team_id,
            caller_team.league_id,
            &incoming_contract_ids,
            db,
        )
        .await?;

        let maybe_processed = accept_trade(
            model.clone(),
            &team_user,
            &Utc::now().fixed_offset(),
            &accommodating_drop_contract_ids,
            TradeLegality::JudgeNow,
            db,
        )
        .await
        .map_err(|err| map_trade_processing_error(&err))?;

        Ok(Trade::from_model(maybe_processed.unwrap_or(model)))
    }

    /// Rejects a trade, closing it for every team involved.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn reject_trade(&self, ctx: &Context<'_>, trade_id: i64) -> Result<Trade> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let model =
            load_actionable_trade(ctx, trade_id, team_user.team_id, caller_team.league_id).await?;

        let rejected = reject_trade(model, &team_user, db)
            .await
            .map_err(|err| internal("failed to reject trade", &err))?;

        Ok(Trade::from_model(rejected))
    }
}

/// Loads a trade the caller may act on: in their league, involving their team, and not superseded.
/// Membership is re-derived from `team_trade` rather than trusted from the request.
async fn load_actionable_trade(
    ctx: &Context<'_>,
    trade_id: i64,
    caller_team_id: i64,
    caller_league_id: i64,
) -> Result<trade::Model> {
    let db = ctx.data_unchecked::<DatabaseConnection>();

    let model = find_trade_by_id(trade_id, db)
        .await
        .map_err(|_| code_error(ErrorCode::NotFound))?;
    if model.league_id != caller_league_id {
        return Err(code_error(ErrorCode::NotFound));
    }

    let involved_teams = model
        .get_teams(db)
        .await
        .map_err(|err| internal("failed to load trade teams", &err))?;
    if !involved_teams.iter().any(|team| team.id == caller_team_id) {
        return Err(code_error(ErrorCode::Forbidden));
    }

    // logic/ re-checks this; checking here is what turns it into a typed error instead of a 500.
    let is_latest = model
        .is_latest_in_chain(db)
        .await
        .map_err(|err| internal("failed to check trade chain", &err))?;
    if !is_latest {
        return Err(code_error(ErrorCode::NotLatestInChain));
    }

    Ok(model)
}

/// Re-derives that every accommodating drop is a contract the submitting owner may drop with this
/// trade: an active one on their roster now, or one this trade brings them (which T2 then refuses
/// at process time, rather than this reading as a request for someone else's player).
async fn validate_accommodating_drops(
    contract_ids: &[i64],
    submitting_team_id: i64,
    league_id: i64,
    incoming_contract_ids: &[i64],
    db: &DatabaseConnection,
) -> Result<()> {
    for contract_id in contract_ids {
        let contract_model = find_contract_by_id(*contract_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?;
        if contract_model.league_id != league_id {
            return Err(code_error(ErrorCode::NotFound));
        }
        if contract_model.status != ContractStatus::Active {
            return Err(graphql_error(
                ErrorCode::BadRequest,
                format!("contract {contract_id} is not active, so it cannot be dropped"),
            ));
        }
        if contract_model.team_id != Some(submitting_team_id)
            && !incoming_contract_ids.contains(contract_id)
        {
            return Err(code_error(ErrorCode::Forbidden));
        }
    }

    Ok(())
}

/// A trade whose teams have no cached pre-trade salary is a data problem the client can report,
/// so it gets its own code rather than a bare server fault. A season missing its lock deadlines is
/// reported the same way owner-facing roster moves report it (see `resolve_upcoming_roster_lock`),
/// and so is a season missing any other deadline row the trade needs.
fn map_trade_processing_error(error: &Report) -> GraphQlError {
    if let Some(missing) = error.downcast_ref::<MissingPreTradeSalary>() {
        return graphql_error(ErrorCode::MissingPreTradeSalary, missing.to_string());
    }
    if let Some(missing) = error.downcast_ref::<MissingUpcomingRosterLock>() {
        return graphql_error(ErrorCode::BadRequest, missing.to_string());
    }
    if let Some(missing) = error.downcast_ref::<MissingSeasonDeadline>() {
        return graphql_error(ErrorCode::BadRequest, missing.to_string());
    }

    // A refused transaction reaches here as a `RosterMoveRejection`: the trade plus one owner's
    // accommodating drops broke T1 or T2, which the owner reads the same way a roster move does.
    roster_move_error(error)
}

fn internal(message: &str, error: &Report) -> GraphQlError {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use fbkl_entity::deadline::DeadlineKind;

    use super::*;

    fn error_code(error: &GraphQlError) -> Option<async_graphql::Value> {
        error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .cloned()
    }

    #[test]
    fn missing_pre_trade_salary_is_typed_not_a_server_fault() {
        let error = map_trade_processing_error(&Report::new(MissingPreTradeSalary { team_id: 7 }));

        assert_eq!(error_code(&error), Some("MISSING_PRE_TRADE_SALARY".into()));
        assert!(error.message.contains("team (id = 7)"));
    }

    #[test]
    fn a_season_with_no_lock_left_is_a_named_failure_not_a_server_fault() {
        let error = map_trade_processing_error(&Report::new(MissingUpcomingRosterLock {
            league_id: 3,
            end_of_season_year: 2026,
        }));

        assert_eq!(error_code(&error), Some("BAD_REQUEST".into()));
        assert!(error.message.contains("no roster lock is still to fire"));
        assert!(error.message.contains("missing lock deadlines"));
    }

    #[test]
    fn a_season_missing_a_deadline_row_is_named_not_a_server_fault() {
        let error = map_trade_processing_error(&Report::new(MissingSeasonDeadline {
            league_id: 3,
            end_of_season_year: 2026,
            kind: DeadlineKind::PreseasonStart,
        }));

        assert_eq!(error_code(&error), Some("BAD_REQUEST".into()));
        assert!(error.message.contains("PreseasonStart"));
    }

    #[test]
    fn other_trade_failures_stay_internal_and_generic() {
        let error = map_trade_processing_error(&color_eyre::eyre::eyre!("db exploded"));

        assert_eq!(error_code(&error), Some("INTERNAL".into()));
        assert_eq!(error.message, "internal server error");
    }
}
