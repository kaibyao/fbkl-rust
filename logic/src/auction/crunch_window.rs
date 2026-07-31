//! Entering the preseason crunch window (rules §6.4.4, spec 01 "Timing rules").
//!
//! In the last 24h before the preseason auctions' hard deadline, a bid's reprieve drops from 24h to
//! 1h so a bidding war actually ends before the final roster lock. Bids placed inside the window
//! already get the short reprieve from `place_auction_bid`; this sweep is what shortens the auctions
//! that were already sitting on a 24h clock when the window opened.
//!
//! Nothing needs re-deriving afterwards and nothing guards against re-firing: the sweep only ever
//! moves a close time earlier, so a second run over the same auctions is a no-op.

use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    auction::{self, AuctionKind},
    auction_queries,
    sea_orm::{ConnectionTrait, TransactionTrait, prelude::DateTimeWithTimeZone},
};
use tracing::instrument;

use super::{auction_close_at, auction_quiet_window, find_auction_mode_deadlines};

/// Shortens every open preseason auction in the league to its last bid + 1h once the crunch window
/// has opened, clamped to the hard deadline. Returns the auctions it moved.
///
/// Unbid auctions are left alone: their clock is the §6.3.4 tier ladder, not a bid's reprieve.
#[instrument(skip(db))]
pub async fn shorten_open_auctions_for_crunch_window<C>(
    league_id: i64,
    end_of_season_year: i16,
    kind: AuctionKind,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<Vec<auction::Model>>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let mode_deadlines =
        find_auction_mode_deadlines(kind, league_id, end_of_season_year, now, db).await?;
    // Nothing to shorten until the window opens, and in-season never has one.
    if mode_deadlines
        .crunch_window_start
        .is_none_or(|crunch_window_start| now < crunch_window_start)
    {
        return Ok(Vec::new());
    }
    let crunch_quiet_window = auction_quiet_window(now, mode_deadlines.crunch_window_start);

    let open_auctions = auction_queries::find_open_auctions_in_league(
        league_id,
        end_of_season_year,
        Some(kind),
        db,
    )
    .await?;

    let db_txn = db.begin().await?;
    let mut shortened_auctions = Vec::new();
    for open_auction in open_auctions {
        let Some(last_bid) = open_auction.get_latest_bid(&db_txn).await? else {
            continue;
        };
        let shortened_close_at = auction_close_at(
            last_bid.created_at,
            crunch_quiet_window,
            open_auction.all_bid_deadline_timestamp,
            mode_deadlines.hard_deadline,
        )?;
        if shortened_close_at < open_auction.close_at_timestamp {
            shortened_auctions.push(
                auction_queries::set_auction_close_at(open_auction.id, shortened_close_at, &db_txn)
                    .await?,
            );
        }
    }
    db_txn.commit().await?;

    Ok(shortened_auctions)
}
