//! Auction reads plus bidding. Auctions open and settle on the scheduler tick, never as a direct
//! mutation — the only auction mutation an owner has is `placeBid`.
//!
//! The commissioner's two per-season veteran-auction inputs (§6.3.6) also live here, since they are
//! what pool assembly reads when the auction-start deadline fires.

use async_graphql::{Context, Error as GraphQlError, Object, Result, SimpleObject};
use chrono::Utc;
use color_eyre::Report;
use fbkl_entity::{
    auction::{self, AuctionKind, AuctionStatus},
    auction_bid,
    auction_queries::{
        find_auction_bids, find_auction_by_id, find_open_auctions_in_league,
        find_winning_bids_for_team,
    },
    auction_schedule_queries::{
        find_auction_schedule_rows_for_season, set_min_bid_tiers, set_veteran_auction_ranking,
        validate_min_bid_tiers,
    },
    deadline::DeadlineKind,
    deadline_queries::find_sorted_deadlines_for_league_season,
    sea_orm::DatabaseConnection,
};
use fbkl_logic::auction::{BidRejection, place_auction_bid};

use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, current_season, deadline::Deadline,
    graphql_error, require_league_role,
};

/// Bid history spans a whole auction, so a page is always bounded (same convention as the
/// transaction feed).
const MAX_PAGE_SIZE: u64 = 100;

/// The deadlines that bound an auction window.
const AUCTION_DEADLINE_KINDS: [DeadlineKind; 6] = [
    DeadlineKind::PreseasonVeteranAuctionStart,
    DeadlineKind::PreseasonFaAuctionStart,
    DeadlineKind::PreseasonFaAuctionEnd,
    DeadlineKind::Week1FreeAgentAuctionStart,
    DeadlineKind::Week1FreeAgentAuctionEnd,
    DeadlineKind::FreeAgentAuctionEnd,
];

/// An auction on one contract. `transactionId` is null while the auction is still open.
#[derive(SimpleObject)]
pub struct Auction {
    pub id: i64,
    pub kind: AuctionKind,
    pub status: AuctionStatus,
    pub minimum_bid_amount: i16,
    pub start_timestamp: String,
    /// When the auction stops taking bids — the countdown to lead with.
    pub close_at_timestamp: String,
    /// In-season FA only: the week's all-bid cutoff, which a late bid rolls +30min (rules §8.3.2). Null for preseason auctions.
    pub all_bid_deadline_timestamp: Option<String>,
    /// RFA/UFA only: the team barred from bidding (rules §6.2.2.3) — disable their bid input.
    pub original_owner_team_id: Option<i64>,
    pub contract_id: i64,
    pub transaction_id: Option<i64>,
}

impl Auction {
    fn from_model(model: &auction::Model) -> Self {
        Self {
            id: model.id,
            kind: model.kind,
            status: model.status,
            minimum_bid_amount: model.minimum_bid_amount,
            start_timestamp: model.start_timestamp.to_rfc3339(),
            close_at_timestamp: model.close_at_timestamp.to_rfc3339(),
            all_bid_deadline_timestamp: model
                .all_bid_deadline_timestamp
                .map(|deadline| deadline.to_rfc3339()),
            original_owner_team_id: model.original_owner_team_id,
            contract_id: model.contract_id,
            transaction_id: model.transaction_id,
        }
    }
}

#[derive(SimpleObject)]
pub struct AuctionBid {
    pub id: i64,
    pub bid_amount: i16,
    pub comment: Option<String>,
    pub auction_id: i64,
    pub team_user_id: i64,
    pub created_at: String,
}

impl AuctionBid {
    fn from_model(model: &auction_bid::Model) -> Self {
        Self {
            id: model.id,
            bid_amount: model.bid_amount,
            comment: model.comment.clone(),
            auction_id: model.auction_id,
            team_user_id: model.team_user_id,
            created_at: model.created_at.to_rfc3339(),
        }
    }
}

/// One page of an auction's bids, newest first.
#[derive(SimpleObject)]
pub struct PagedAuctionBids {
    pub items: Vec<AuctionBid>,
    pub total_items: u64,
}

/// One of the caller's currently-winning bids, for the committed-cap display (rules §6.4.1).
#[derive(SimpleObject)]
pub struct WinningBid {
    pub auction_id: i64,
    pub bid_amount: i16,
}

#[derive(Default)]
pub struct AuctionQuery;

#[Object]
impl AuctionQuery {
    /// Auctions in the caller's league still taking bids, soonest close first.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn open_auctions(&self, ctx: &Context<'_>) -> Result<Vec<Auction>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let season = current_season(ctx, caller_team.league_id).await?;

        let auctions = find_open_auctions_in_league(caller_team.league_id, season, None, db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to load open auctions");
                code_error(ErrorCode::Internal)
            })?;

        Ok(auctions.iter().map(Auction::from_model).collect())
    }

    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn auction(&self, ctx: &Context<'_>, id: i64) -> Result<Auction> {
        let model = load_auction_in_league(ctx, id).await?;
        Ok(Auction::from_model(&model))
    }

    /// One page of an auction's bid history, newest bid first.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn bid_history(
        &self,
        ctx: &Context<'_>,
        auction_id: i64,
        #[graphql(default = 0)] page: u64,
        #[graphql(default = 25)] page_size: u64,
    ) -> Result<PagedAuctionBids> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        load_auction_in_league(ctx, auction_id).await?;

        let paged = find_auction_bids(auction_id, page, page_size.min(MAX_PAGE_SIZE), db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, auction_id, "failed to load auction bids");
                code_error(ErrorCode::Internal)
            })?;

        Ok(PagedAuctionBids {
            items: paged.items.iter().map(AuctionBid::from_model).collect(),
            total_items: paged.total_items,
        })
    }

    /// The caller's team's currently-winning bids in this season's open auctions, plus the RFA
    /// auctions it won that are still in the raise/match handshake (rules §15.3.4).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn my_winning_bids(&self, ctx: &Context<'_>) -> Result<Vec<WinningBid>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let season = current_season(ctx, caller_team.league_id).await?;

        let bids = find_winning_bids_for_team(team_user.team_id, caller_team.league_id, season, db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to load winning bids");
                code_error(ErrorCode::Internal)
            })?;

        Ok(bids
            .into_iter()
            .map(|(auction_id, bid_amount)| WinningBid {
                auction_id,
                bid_amount,
            })
            .collect())
    }

    /// The season's auction-window boundaries, oldest first. Defaults to the current season.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn auction_schedule(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Vec<Deadline>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

        let season = match end_of_season_year {
            Some(year) => year,
            None => current_season(ctx, caller_team.league_id).await?,
        };

        let deadlines = find_sorted_deadlines_for_league_season(caller_team.league_id, season, db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to load the auction schedule");
                code_error(ErrorCode::Internal)
            })?;

        Ok(deadlines
            .iter()
            .filter(|deadline| AUCTION_DEADLINE_KINDS.contains(&deadline.kind))
            .map(Deadline::from_model)
            .collect())
    }
}

#[derive(Default)]
pub struct AuctionMutation;

#[Object]
impl AuctionMutation {
    /// Places a bid for the caller's own team. The bidding team comes from the session, never the
    /// client, and every rejection reason carries its own error code.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn place_bid(
        &self,
        ctx: &Context<'_>,
        auction_id: i64,
        bid_amount: i16,
        comment: Option<String>,
    ) -> Result<AuctionBid> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (team_user, _) = require_league_role(ctx, RoleRequirement::Member).await?;
        load_auction_in_league(ctx, auction_id).await?;

        let bid = place_auction_bid(
            auction_id,
            team_user.id,
            bid_amount,
            comment,
            Utc::now().into(),
            db,
        )
        .await
        .map_err(|err| bid_error(&err))?;

        Ok(AuctionBid::from_model(&bid))
    }

    /// Sets the current season's veteran-auction minimum-bid tiers, top tier first (rules §6.3.6).
    /// Replaces any tiers already entered, so re-entry is idempotent.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn set_veteran_auction_min_bid_tiers(
        &self,
        ctx: &Context<'_>,
        min_bid_amounts: Vec<i16>,
    ) -> Result<Vec<i16>> {
        validate_min_bid_tiers(&min_bid_amounts)
            .map_err(|err| graphql_error(ErrorCode::BadRequest, err.to_string()))?;

        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;
        let season = current_season(ctx, caller_team.league_id).await?;
        ensure_veteran_auction_not_started(caller_team.league_id, season, db).await?;

        let tiers = set_min_bid_tiers(caller_team.league_id, season, &min_bid_amounts, db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to set the minimum bid tiers");
                code_error(ErrorCode::Internal)
            })?;

        Ok(tiers.iter().map(|tier| tier.min_bid_amount).collect())
    }

    /// Sets the current season's ranked veteran-auction nomination list, best player first
    /// (rules §6.3.2). Replaces any list already entered, so re-entry is idempotent.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn set_veteran_auction_ranking(
        &self,
        ctx: &Context<'_>,
        player_ids: Vec<i64>,
    ) -> Result<Vec<i64>> {
        if player_ids.is_empty() {
            return Err(graphql_error(
                ErrorCode::BadRequest,
                "a ranked veteran auction list needs at least one player",
            ));
        }

        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;
        let season = current_season(ctx, caller_team.league_id).await?;
        ensure_veteran_auction_not_started(caller_team.league_id, season, db).await?;

        set_veteran_auction_ranking(caller_team.league_id, season, &player_ids, db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to set the veteran auction ranking");
                code_error(ErrorCode::Internal)
            })
    }
}

/// §6.3.6 config is set before the auction starts; assembled schedule rows already carry tier
/// assignments, so a late rewrite would desync them.
async fn ensure_veteran_auction_not_started(
    league_id: i64,
    season: i16,
    db: &DatabaseConnection,
) -> Result<()> {
    let schedule_rows = find_auction_schedule_rows_for_season(league_id, season, db)
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to check the veteran auction schedule");
            code_error(ErrorCode::Internal)
        })?;

    if schedule_rows.is_empty() {
        Ok(())
    } else {
        Err(code_error(ErrorCode::VeteranAuctionStarted))
    }
}

/// A refused bid is the client's fault and gets its own code; anything else is a server fault.
fn bid_error(error: &Report) -> GraphQlError {
    let Some(rejection) = error.downcast_ref::<BidRejection>() else {
        tracing::error!(error = ?error, "failed to place a bid");
        return code_error(ErrorCode::Internal);
    };

    let code = match rejection {
        BidRejection::AuctionClosed { .. } | BidRejection::BiddingWindowElapsed { .. } => {
            ErrorCode::AuctionNotOpen
        }
        BidRejection::OriginalOwner => ErrorCode::BidOriginalOwner,
        BidRejection::BelowMinimum { .. } => ErrorCode::BidBelowMinimum,
        BidRejection::BelowIncrement { .. } => ErrorCode::BidBelowIncrement,
        BidRejection::InsufficientCap { .. } => ErrorCode::BidInsufficientCap,
        BidRejection::NoRosterSpace { .. } => ErrorCode::BidNoRosterSpace,
        BidRejection::MissingCompensationPick { .. } => ErrorCode::BidMissingCompensationPick,
    };

    graphql_error(code, rejection.to_string())
}

/// An auction only reaches a league through its contract, so scoping needs that extra hop.
async fn load_auction_in_league(ctx: &Context<'_>, auction_id: i64) -> Result<auction::Model> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;

    let auction_model = find_auction_by_id(auction_id, db)
        .await
        .map_err(|_| code_error(ErrorCode::NotFound))?;
    let contract_model = auction_model
        .get_contract(db)
        .await
        .map_err(|_| code_error(ErrorCode::NotFound))?;

    if contract_model.league_id != caller_team.league_id {
        return Err(code_error(ErrorCode::NotFound));
    }

    Ok(auction_model)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(error: &GraphQlError) -> Option<async_graphql::Value> {
        error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .cloned()
    }

    #[test]
    fn every_bid_rejection_gets_its_own_code() {
        let cases = [
            (
                BidRejection::AuctionClosed {
                    auction_id: 1,
                    status: AuctionStatus::Completed,
                },
                "AUCTION_NOT_OPEN",
            ),
            (BidRejection::OriginalOwner, "BID_ORIGINAL_OWNER"),
            (
                BidRejection::BelowMinimum {
                    bid_amount: 1,
                    minimum_bid_amount: 5,
                },
                "BID_BELOW_MINIMUM",
            ),
            (
                BidRejection::BelowIncrement {
                    bid_amount: 5,
                    required: 6,
                },
                "BID_BELOW_INCREMENT",
            ),
            (
                BidRejection::InsufficientCap {
                    bid_amount: 5,
                    committed_salary: 200,
                    salary_cap: 100,
                },
                "BID_INSUFFICIENT_CAP",
            ),
            (
                BidRejection::NoRosterSpace {
                    roster_used: 16,
                    roster_limit: 15,
                },
                "BID_NO_ROSTER_SPACE",
            ),
            (
                BidRejection::MissingCompensationPick {
                    bid_amount: 19,
                    required_round: 3,
                },
                "BID_MISSING_COMPENSATION_PICK",
            ),
        ];

        for (rejection, expected_code) in cases {
            let error = bid_error(&Report::new(rejection));
            assert_eq!(code_of(&error), Some(expected_code.into()));
        }
    }

    #[test]
    fn other_failures_stay_internal() {
        let error = bid_error(&color_eyre::eyre::eyre!("db exploded"));

        assert_eq!(code_of(&error), Some("INTERNAL".into()));
    }
}
