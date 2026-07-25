//! Read-only auction surface. Bidding (`placeBid`) lands with fbkl-rust-lcc (spec 01); auctions
//! open and settle through deadline processing, never as a direct mutation.

use async_graphql::{Context, Object, Result, SimpleObject};
use chrono::Utc;
use fbkl_entity::{
    auction::{self, AuctionKind},
    auction_bid,
    auction_queries::{find_auction_bids, find_auction_by_id, find_open_auctions_in_league},
    deadline::DeadlineKind,
    deadline_queries::{
        find_most_recent_deadline_by_datetime, find_sorted_deadlines_for_league_season,
    },
    sea_orm::DatabaseConnection,
};

use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, deadline::Deadline,
    require_league_role,
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
    pub minimum_bid_amount: i16,
    pub start_timestamp: String,
    pub soft_end_timestamp: String,
    pub fixed_end_timestamp: String,
    pub contract_id: i64,
    pub transaction_id: Option<i64>,
}

impl Auction {
    fn from_model(model: &auction::Model) -> Self {
        Self {
            id: model.id,
            kind: model.kind,
            minimum_bid_amount: model.minimum_bid_amount,
            start_timestamp: model.start_timestamp.to_rfc3339(),
            soft_end_timestamp: model.soft_end_timestamp.to_rfc3339(),
            fixed_end_timestamp: model.fixed_end_timestamp.to_rfc3339(),
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

#[derive(Default)]
pub struct AuctionQuery;

#[Object]
impl AuctionQuery {
    /// Auctions in the caller's league that have not settled yet, soonest deadline first.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn open_auctions(&self, ctx: &Context<'_>) -> Result<Vec<Auction>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
        let season = current_season(ctx, caller_team.league_id).await?;

        let auctions = find_open_auctions_in_league(caller_team.league_id, season, db)
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

async fn current_season(ctx: &Context<'_>, league_id: i64) -> Result<i16> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let deadline = find_most_recent_deadline_by_datetime(league_id, Utc::now().fixed_offset(), db)
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, league_id, "failed to resolve the current season");
            code_error(ErrorCode::Internal)
        })?;

    Ok(deadline.end_of_season_year)
}
