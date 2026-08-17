//! The restricted free agent handshake (spec 03, rules §15.2, §15.3): the winner's 48-hour raise
//! period, then the original owner's 48-hour match-or-decline period.
//!
//! Every rule lives in `fbkl_logic::deadline_processing`; these resolvers only authorize, fetch and
//! map. The acting team always comes from the session, so the logic layer's own team checks are
//! what reject a caller acting for somebody else.
//!
//! Both the projected re-sign price and the compensation tier are computed here from backend code,
//! never re-derived by the client (spec 04).

use async_graphql::{ComplexObject, Context, Error as GraphQlError, Object, Result, SimpleObject};
use chrono::Utc;
use color_eyre::Report;
use fbkl_constants::league_rules::compensation_round_for_bid;
use fbkl_entity::{
    contract::FreeAgentException,
    contract_queries::find_contract_by_id,
    rfa_compensation_pick,
    rfa_resolution::{self, RfaResolutionStatus},
    rfa_resolution_queries::{
        find_rfa_compensation_pick_for_resolution, find_rfa_resolution_by_id,
        find_rfa_resolution_for_contract, find_rfa_resolutions_for_league_season,
    },
    sea_orm::DatabaseConnection,
    team_user,
};
use fbkl_logic::deadline_processing::{
    RfaMatchDecision, UnbidRfaDecision, compute_eligible_compensation_picks, decline_to_raise,
    match_or_decline, raise_bid, resolve_unbid_rfa,
};

use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, current_season, draft::DraftPick,
    graphql_error, require_league_role,
};

/// One restricted free agent's place in the raise/match handshake (rules §15.3).
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct RfaResolution {
    pub id: i64,
    pub end_of_season_year: i16,
    pub status: RfaResolutionStatus,
    pub rfa_contract_id: i64,
    /// The team that held the player at the keeper deadline; it holds the discount right (rules §15.4.2).
    pub original_owner_team_id: i64,
    /// Null while the player's auction has not closed, and for a player nobody bid on.
    pub auction_id: Option<i64>,
    pub winning_team_id: Option<i64>,
    pub final_bid: Option<i16>,
    pub raised_bid: Option<i16>,
    /// The raise when the winner raised, else the winning bid — the price the owner would match.
    pub effective_bid: Option<i16>,
    /// The best round a forfeited pick may be, given the effective bid (rules §15.2.1).
    pub compensation_round: Option<i16>,
    pub final_bid_at: Option<String>,
    /// Auction close + 48h; the countdown for the winner.
    pub raise_deadline_at: Option<String>,
    /// Raise settled + 48h; the countdown for the original owner.
    pub match_deadline_at: Option<String>,
    pub resolved_at: Option<String>,
    #[graphql(skip)]
    model: rfa_resolution::Model,
}

impl RfaResolution {
    fn from_model(model: &rfa_resolution::Model) -> Self {
        Self {
            id: model.id,
            end_of_season_year: model.end_of_season_year,
            status: model.status,
            rfa_contract_id: model.rfa_contract_id,
            original_owner_team_id: model.original_owner_team_id,
            auction_id: model.auction_id,
            winning_team_id: model.winning_team_id,
            final_bid: model.final_bid,
            raised_bid: model.raised_bid,
            effective_bid: model.effective_bid(),
            compensation_round: model.effective_bid().map(compensation_round_for_bid),
            final_bid_at: model.final_bid_at.map(|at| at.to_rfc3339()),
            raise_deadline_at: model.raise_deadline_at.map(|at| at.to_rfc3339()),
            match_deadline_at: model.match_deadline_at.map(|at| at.to_rfc3339()),
            resolved_at: model.resolved_at.map(|at| at.to_rfc3339()),
            model: model.clone(),
        }
    }
}

#[ComplexObject]
impl RfaResolution {
    /// What the original owner would pay to re-sign at its discount (rules §15.3.2, §15.3.5).
    ///
    /// Null once the handshake is over, because the contract is no longer a free agent one by then
    /// and there is no price left to quote.
    async fn projected_resign_salary(&self, ctx: &Context<'_>) -> Result<Option<i16>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let rfa_contract = find_contract_by_id(self.model.rfa_contract_id, db)
            .await
            .map_err(|err| internal("failed to load the RFA contract", &err))?
            // A trade during the auction replaces the row the resolution points at.
            .get_latest_in_chain(db)
            .await
            .map_err(|err| internal("failed to load the RFA contract", &err))?;
        // No bid means no price to match, so the owner discounts the carry salary instead (rules §15.3.5).
        let (signing_amount, fa_exception) = self.model.effective_bid().map_or(
            (rfa_contract.salary, FreeAgentException::HeldNoBid),
            |effective_bid| (effective_bid, FreeAgentException::Held),
        );
        let Ok(resigned_contract) = rfa_contract.sign_rfa_or_ufa_contract_to_team(
            self.model.original_owner_team_id,
            signing_amount,
            fa_exception,
        ) else {
            return Ok(None);
        };

        Ok(resigned_contract.salary.try_as_ref().copied())
    }

    /// The picks the winner may forfeit on a decline, best round first (rules §15.2).
    ///
    /// Empty until the auction closes, and once the handshake is over.
    async fn eligible_compensation_picks(&self, ctx: &Context<'_>) -> Result<Vec<DraftPick>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        if !matches!(
            self.model.status,
            RfaResolutionStatus::AwaitingRaise | RfaResolutionStatus::AwaitingMatch
        ) {
            return Ok(vec![]);
        }

        let draft_picks = compute_eligible_compensation_picks(&self.model, db)
            .await
            .map_err(|err| internal("failed to compute the eligible compensation picks", &err))?;

        Ok(draft_picks.iter().map(DraftPick::from_model).collect())
    }

    /// The pick a decline owed, once one was declined. Null in every other state.
    async fn compensation_pick(&self, ctx: &Context<'_>) -> Result<Option<RfaCompensationPick>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let maybe_pick = find_rfa_compensation_pick_for_resolution(self.model.id, db)
            .await
            .map_err(|err| internal("failed to load the compensation pick", &err))?;

        Ok(maybe_pick.as_ref().map(RfaCompensationPick::from_model))
    }
}

/// The draft pick a declined RFA owes the original owner (rules §15.2).
#[derive(SimpleObject)]
pub struct RfaCompensationPick {
    pub id: i64,
    pub rfa_resolution_id: i64,
    pub required_round: i16,
    pub forfeited_draft_pick_id: Option<i64>,
    /// The original owner, which receives the pick.
    pub to_team_id: i64,
    /// The winning bidder, which gives up the pick.
    pub from_team_id: i64,
}

impl RfaCompensationPick {
    const fn from_model(model: &rfa_compensation_pick::Model) -> Self {
        Self {
            id: model.id,
            rfa_resolution_id: model.rfa_resolution_id,
            required_round: model.required_round,
            forfeited_draft_pick_id: model.forfeited_draft_pick_id,
            to_team_id: model.to_team_id,
            from_team_id: model.from_team_id,
        }
    }
}

#[derive(Default)]
pub struct RfaQuery;

#[Object]
impl RfaQuery {
    /// Every restricted free agent resolution in the league season, oldest first. Defaults to the
    /// current season.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn rfa_resolutions(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Vec<RfaResolution>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let season = match end_of_season_year {
            Some(year) => year,
            None => current_season(ctx, caller_team.league_id).await?,
        };
        let rfa_resolutions =
            find_rfa_resolutions_for_league_season(caller_team.league_id, season, db)
                .await
                .map_err(|err| internal("failed to load the RFA resolutions", &err))?;

        Ok(rfa_resolutions
            .iter()
            .map(RfaResolution::from_model)
            .collect())
    }

    /// The resolution for one RFA contract, if that player was designated. A trade replaces the
    /// contract row, so any contract in the same season's chain finds it.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn rfa_resolution(
        &self,
        ctx: &Context<'_>,
        contract_id: i64,
    ) -> Result<Option<RfaResolution>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let maybe_rfa_resolution = find_rfa_resolution_for_contract(contract_id, db)
            .await
            .map_err(|_| code_error(ErrorCode::NotFound))?
            .filter(|rfa_resolution| rfa_resolution.league_id == caller_team.league_id);

        Ok(maybe_rfa_resolution.as_ref().map(RfaResolution::from_model))
    }
}

#[derive(Default)]
pub struct RfaMutation;

#[Object]
impl RfaMutation {
    /// The winning bidder raises its own bid once, which opens the original owner's period
    /// straight away (rules §15.3.2.1).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn raise_rfa_bid(
        &self,
        ctx: &Context<'_>,
        rfa_resolution_id: i64,
        bid_amount: i16,
    ) -> Result<RfaResolution> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user = load_acting_team_user(ctx, rfa_resolution_id).await?;

        let raised = raise_bid(
            rfa_resolution_id,
            team_user.team_id,
            bid_amount,
            Utc::now().into(),
            db,
        )
        .await
        .map_err(|err| refused("failed to raise the RFA bid", &err))?;

        Ok(RfaResolution::from_model(&raised))
    }

    /// The winning bidder stands pat, which opens the original owner's period straight away
    /// instead of waiting the full 48 hours out (rules §15.3.2.1).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn decline_to_raise_rfa(
        &self,
        ctx: &Context<'_>,
        rfa_resolution_id: i64,
    ) -> Result<RfaResolution> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user = load_acting_team_user(ctx, rfa_resolution_id).await?;

        let settled = decline_to_raise(rfa_resolution_id, team_user.team_id, Utc::now().into(), db)
            .await
            .map_err(|err| refused("failed to settle the RFA raise period", &err))?;

        Ok(RfaResolution::from_model(&settled))
    }

    /// The original owner matches the effective bid and re-signs the player at its discount
    /// (rules §15.3.2).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn match_rfa(&self, ctx: &Context<'_>, rfa_resolution_id: i64) -> Result<RfaResolution> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user = load_acting_team_user(ctx, rfa_resolution_id).await?;

        let matched = match_or_decline(
            rfa_resolution_id,
            team_user.team_id,
            RfaMatchDecision::Match,
            None,
            Utc::now().into(),
            db,
        )
        .await
        .map_err(|err| refused("failed to match the RFA bid", &err))?;

        Ok(RfaResolution::from_model(&matched))
    }

    /// The original owner declines: the winner signs the player and forfeits a pick (rules §15.2).
    /// Leaving `forfeitedDraftPickId` null forfeits the cheapest eligible pick.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn decline_rfa(
        &self,
        ctx: &Context<'_>,
        rfa_resolution_id: i64,
        forfeited_draft_pick_id: Option<i64>,
    ) -> Result<RfaResolution> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user = load_acting_team_user(ctx, rfa_resolution_id).await?;

        let declined = match_or_decline(
            rfa_resolution_id,
            team_user.team_id,
            RfaMatchDecision::Decline,
            forfeited_draft_pick_id,
            Utc::now().into(),
            db,
        )
        .await
        .map_err(|err| refused("failed to decline the RFA bid", &err))?;

        Ok(RfaResolution::from_model(&declined))
    }

    /// Nobody bid: the original owner re-signs at the standard 4th-year salary (rules §15.3.5).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn resign_unbid_rfa(
        &self,
        ctx: &Context<'_>,
        rfa_resolution_id: i64,
    ) -> Result<RfaResolution> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user = load_acting_team_user(ctx, rfa_resolution_id).await?;

        let resigned = resolve_unbid_rfa(
            rfa_resolution_id,
            team_user.team_id,
            UnbidRfaDecision::Resign,
            Utc::now().into(),
            db,
        )
        .await
        .map_err(|err| refused("failed to re-sign the unbid RFA", &err))?;

        Ok(RfaResolution::from_model(&resigned))
    }

    /// Nobody bid: the original owner passes, sending the player to the regular free agent auction
    /// (rules §15.3.5).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn release_unbid_rfa_to_auction(
        &self,
        ctx: &Context<'_>,
        rfa_resolution_id: i64,
    ) -> Result<RfaResolution> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_user = load_acting_team_user(ctx, rfa_resolution_id).await?;

        let released = resolve_unbid_rfa(
            rfa_resolution_id,
            team_user.team_id,
            UnbidRfaDecision::ReleaseToAuction,
            Utc::now().into(),
            db,
        )
        .await
        .map_err(|err| refused("failed to release the unbid RFA", &err))?;

        Ok(RfaResolution::from_model(&released))
    }
}

/// Resolves the caller's `team_user` and checks the resolution is in the caller's league.
///
/// Which of the two teams may act right now is the logic layer's call, so this only proves the
/// resolution is visible to the caller at all.
async fn load_acting_team_user(
    ctx: &Context<'_>,
    rfa_resolution_id: i64,
) -> Result<team_user::Model> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

    let rfa_resolution = find_rfa_resolution_by_id(rfa_resolution_id, db)
        .await
        .map_err(|_| code_error(ErrorCode::NotFound))?;
    if rfa_resolution.league_id != caller_team.league_id {
        return Err(code_error(ErrorCode::NotFound));
    }

    Ok(team_user)
}

/// A refused handshake move is the caller's fault: wrong team, closed period, or a raise the
/// caller cannot pay compensation for.
fn refused(context: &str, error: &Report) -> GraphQlError {
    tracing::warn!(error = ?error, context);
    graphql_error(ErrorCode::BadRequest, error.to_string())
}

fn internal(context: &str, error: &Report) -> GraphQlError {
    tracing::error!(error = ?error, context);
    code_error(ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use fbkl_entity::sea_orm::prelude::DateTimeWithTimeZone;

    use super::*;

    fn at(rfc3339: &str) -> DateTimeWithTimeZone {
        DateTimeWithTimeZone::parse_from_rfc3339(rfc3339).unwrap()
    }

    fn raised_rfa_resolution() -> rfa_resolution::Model {
        rfa_resolution::Model {
            id: 7,
            league_id: 1,
            end_of_season_year: 2026,
            rfa_contract_id: 42,
            original_owner_team_id: 3,
            auction_id: Some(99),
            winning_team_id: Some(4),
            final_bid: Some(19),
            final_bid_at: Some(at("2025-09-11T12:00:00-05:00")),
            status: RfaResolutionStatus::AwaitingMatch,
            raised_bid: Some(30),
            raise_deadline_at: Some(at("2025-09-13T12:00:00-05:00")),
            match_deadline_at: Some(at("2025-09-15T12:00:00-05:00")),
            resolved_at: None,
            created_at: at("2025-09-01T12:00:00-05:00"),
            updated_at: at("2025-09-11T12:00:00-05:00"),
        }
    }

    #[test]
    fn a_raise_sets_the_price_to_match_and_its_compensation_tier() {
        let resolution = RfaResolution::from_model(&raised_rfa_resolution());

        assert_eq!(resolution.effective_bid, Some(30));
        assert_eq!(resolution.compensation_round, Some(2));
    }

    #[test]
    fn an_unraised_win_is_priced_at_the_winning_bid() {
        let mut model = raised_rfa_resolution();
        model.raised_bid = None;

        let resolution = RfaResolution::from_model(&model);

        assert_eq!(resolution.effective_bid, Some(19));
        assert_eq!(resolution.compensation_round, Some(3));
    }

    #[test]
    fn deadlines_go_out_as_rfc3339_strings() {
        let resolution = RfaResolution::from_model(&raised_rfa_resolution());

        assert_eq!(
            resolution.raise_deadline_at.as_deref(),
            Some("2025-09-13T12:00:00-05:00")
        );
        assert_eq!(
            resolution.match_deadline_at.as_deref(),
            Some("2025-09-15T12:00:00-05:00")
        );
        assert_eq!(resolution.resolved_at, None);
    }

    #[test]
    fn an_unbid_player_has_no_price_and_no_tier() {
        let mut model = raised_rfa_resolution();
        model.auction_id = None;
        model.winning_team_id = None;
        model.final_bid = None;
        model.raised_bid = None;
        model.status = RfaResolutionStatus::AwaitingAuction;

        let resolution = RfaResolution::from_model(&model);

        assert_eq!(resolution.effective_bid, None);
        assert_eq!(resolution.compensation_round, None);
    }
}
