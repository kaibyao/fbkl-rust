//! Placing a bid on a live auction (rules §6.4 for the preseason veteran auction, §8.3 for
//! in-season free agency).
//!
//! A bid is not a roster state change, so unlike most of this crate it records no transaction and
//! no `team_update` — only the eventual auction win (`sign_auction_contract_to_team`) does that.
//! Rejections are returned as [`BidRejection`] so the GraphQL layer can map each reason to a
//! distinct error code instead of a generic 500.

use std::fmt::Debug;

use chrono::TimeDelta;
use color_eyre::{Result, eyre::eyre};
use fbkl_entity::{
    auction::{AuctionKind, AuctionStatus},
    auction_bid, auction_queries,
    sea_orm::{ConnectionTrait, TransactionTrait, prelude::DateTimeWithTimeZone},
    team_user_queries,
};
use tracing::instrument;

/// Why a bid was refused. Each variant is a distinct user-facing rejection reason.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BidRejection {
    #[error("Auction {auction_id} is not accepting bids (status: {status:?}).")]
    AuctionClosed {
        auction_id: i64,
        status: AuctionStatus,
    },
    #[error("Auction {auction_id} stopped accepting bids at {deadline}.")]
    BiddingWindowElapsed {
        auction_id: i64,
        deadline: DateTimeWithTimeZone,
    },
    #[error("A player's previous team cannot bid on that player's own free agent auction.")]
    OriginalOwner,
    #[error("Bid of ${bid_amount} is below the auction's minimum bid of ${minimum_bid_amount}.")]
    BelowMinimum {
        bid_amount: i16,
        minimum_bid_amount: i16,
    },
    #[error("Bid of ${bid_amount} must be at least ${required} (previous bid + $1).")]
    BelowIncrement { bid_amount: i16, required: i16 },
}

/// Places a bid on an open auction, rolling the 24h soft end forward (§6.4.4 / §8.3.1).
#[instrument]
pub async fn place_auction_bid<C>(
    auction_id: i64,
    bidding_team_user_id: i64,
    bid_amount: i16,
    comment: Option<String>,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<auction_bid::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let db_txn = db.begin().await?;

    let auction_model = auction_queries::find_auction_by_id_for_update(auction_id, &db_txn).await?;
    if auction_model.status != AuctionStatus::Open {
        return Err(BidRejection::AuctionClosed {
            auction_id,
            status: auction_model.status,
        }
        .into());
    }
    if now >= auction_model.soft_end_timestamp {
        return Err(BidRejection::BiddingWindowElapsed {
            auction_id,
            deadline: auction_model.soft_end_timestamp,
        }
        .into());
    }
    // Veteran auctions close purely on the 24h soft end; only FA uses fixed_end as a hard deadline.
    if auction_model.kind == AuctionKind::InSeasonFreeAgent
        && now >= auction_model.fixed_end_timestamp
    {
        return Err(BidRejection::BiddingWindowElapsed {
            auction_id,
            deadline: auction_model.fixed_end_timestamp,
        }
        .into());
    }

    let bidding_team_user =
        team_user_queries::find_team_user_by_id(bidding_team_user_id, &db_txn).await?;
    if auction_model.original_owner_team_id == Some(bidding_team_user.team_id) {
        return Err(BidRejection::OriginalOwner.into());
    }

    let maybe_latest_bid = auction_model.get_latest_bid(&db_txn).await?;
    validate_bid_amount(
        bid_amount,
        auction_model.minimum_bid_amount,
        maybe_latest_bid.as_ref().map(|bid| bid.bid_amount),
    )?;

    let inserted_bid = auction_queries::insert_auction_bid(
        auction_id,
        bidding_team_user_id,
        bid_amount,
        comment,
        &db_txn,
    )
    .await?;

    let new_soft_end = now
        .checked_add_signed(TimeDelta::hours(24))
        .ok_or_else(|| eyre!("bid time + 24h overflowed: {now}"))?;
    auction_queries::extend_auction_soft_end(auction_id, new_soft_end, &db_txn).await?;

    db_txn.commit().await?;

    Ok(inserted_bid)
}

/// Opening-bid and $1-increment rules (§6.4.2-.3 / §8.3.3-.4).
const fn validate_bid_amount(
    bid_amount: i16,
    minimum_bid_amount: i16,
    maybe_previous_bid_amount: Option<i16>,
) -> Result<(), BidRejection> {
    match maybe_previous_bid_amount {
        None if bid_amount < minimum_bid_amount => Err(BidRejection::BelowMinimum {
            bid_amount,
            minimum_bid_amount,
        }),
        Some(previous_bid_amount) if bid_amount < previous_bid_amount + 1 => {
            Err(BidRejection::BelowIncrement {
                bid_amount,
                required: previous_bid_amount + 1,
            })
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{BidRejection, validate_bid_amount};

    #[test]
    fn opening_bid_must_reach_the_minimum() {
        assert_eq!(
            validate_bid_amount(4, 5, None),
            Err(BidRejection::BelowMinimum {
                bid_amount: 4,
                minimum_bid_amount: 5
            })
        );
        assert_eq!(validate_bid_amount(5, 5, None), Ok(()));
        assert_eq!(validate_bid_amount(50, 5, None), Ok(()));
    }

    #[test]
    fn subsequent_bids_must_raise_by_at_least_a_dollar() {
        assert_eq!(
            validate_bid_amount(5, 5, Some(5)),
            Err(BidRejection::BelowIncrement {
                bid_amount: 5,
                required: 6
            })
        );
        assert_eq!(validate_bid_amount(6, 5, Some(5)), Ok(()));
        // the minimum bid no longer applies once someone has bid
        assert_eq!(validate_bid_amount(3, 5, Some(2)), Ok(()));
    }
}
