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
use fbkl_constants::league_rules::PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT;
use fbkl_entity::{
    auction::{self, AuctionKind, AuctionStatus},
    auction_bid, auction_queries, contract_queries,
    sea_orm::{ConnectionTrait, TransactionTrait, prelude::DateTimeWithTimeZone},
    team_user_queries,
};
use tracing::instrument;

use crate::roster;

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
    #[error(
        "Bid of ${bid_amount} would commit ${committed_salary} against a ${salary_cap} salary cap."
    )]
    InsufficientCap {
        bid_amount: i16,
        committed_salary: i32,
        salary_cap: i16,
    },
    #[error(
        "Winning this bid would need {roster_used} roster spots, but the limit is {roster_limit}."
    )]
    NoRosterSpace { roster_used: i32, roster_limit: i16 },
}

/// Places a bid on an open auction, rolling its 24h close time forward (§6.4.4 / §8.3.1).
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
    if now >= auction_model.close_at_timestamp {
        return Err(BidRejection::BiddingWindowElapsed {
            auction_id,
            deadline: auction_model.close_at_timestamp,
        }
        .into());
    }
    // In-season FA only; the preseason auctions have no all-bid deadline (rules §8.2.2).
    if let Some(all_bid_deadline) = auction_model.all_bid_deadline_timestamp
        && now >= all_bid_deadline
    {
        return Err(BidRejection::BiddingWindowElapsed {
            auction_id,
            deadline: all_bid_deadline,
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

    validate_bid_cap_and_roster(
        &auction_model,
        bidding_team_user.team_id,
        bid_amount,
        now,
        &db_txn,
    )
    .await?;

    let inserted_bid = auction_queries::insert_auction_bid(
        auction_id,
        bidding_team_user_id,
        bid_amount,
        comment,
        &db_txn,
    )
    .await?;

    // The §8.3.2 last-hour reprieve is a close condition, not a close-time mutation.
    let new_close_at = now
        .checked_add_signed(TimeDelta::hours(24))
        .ok_or_else(|| eyre!("bid time + 24h overflowed: {now}"))?;
    auction_queries::set_auction_close_at(auction_id, new_close_at, &db_txn).await?;

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

/// The rules §6.4.1 "null and void" check: a bid that would break the bidder's cap or roster limit
/// is rejected so the previous bid stays winning.
///
/// Only the preseason veteran auction is gated — §8.3.5 lets in-season FA bidders exceed their free
/// cap and accommodate the win via drops/trades.
#[instrument]
async fn validate_bid_cap_and_roster<C>(
    auction_model: &auction::Model,
    bidding_team_id: i64,
    bid_amount: i16,
    now: DateTimeWithTimeZone,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    if !requires_cap_and_roster_check(auction_model.kind) {
        return Ok(());
    }

    let auctioned_contract = auction_model.get_contract(db).await?;
    let winning_bids = auction_queries::find_winning_bids_for_team(
        bidding_team_id,
        auctioned_contract.league_id,
        auctioned_contract.end_of_season_year,
        db,
    )
    .await?;

    let salary_snapshot = roster::calculate_team_contract_salary_at_datetime(
        auctioned_contract.league_id,
        bidding_team_id,
        now,
        db,
    )
    .await?;
    let committed_salary = committed_salary(
        salary_snapshot.salary,
        &winning_bids,
        auction_model.id,
        bid_amount,
    );
    if committed_salary > i32::from(salary_snapshot.cap) {
        return Err(BidRejection::InsufficientCap {
            bid_amount,
            committed_salary,
            salary_cap: salary_snapshot.cap,
        }
        .into());
    }

    let active_contracts =
        contract_queries::find_active_contracts_for_team(bidding_team_id, db).await?;
    let roster_used = roster_spots_used(active_contracts.len(), &winning_bids, auction_model.id);
    if roster_used > i32::from(PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT) {
        return Err(BidRejection::NoRosterSpace {
            roster_used,
            roster_limit: PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT,
        }
        .into());
    }

    Ok(())
}

const fn requires_cap_and_roster_check(kind: AuctionKind) -> bool {
    matches!(kind, AuctionKind::PreseasonVeteranAuction)
}

/// Salary the bidder would be committed to if this bid wins. Re-bidding on an auction the team
/// already leads swaps the old amount for the new one instead of counting both.
fn committed_salary(
    team_current_salary: i16,
    winning_bids: &[(i64, i16)],
    this_auction_id: i64,
    bid_amount: i16,
) -> i32 {
    let other_winning_bids: i32 = winning_bids
        .iter()
        .filter(|(auction_id, _)| *auction_id != this_auction_id)
        .map(|(_, amount)| i32::from(*amount))
        .sum();
    i32::from(team_current_salary) + other_winning_bids + i32::from(bid_amount)
}

/// Roster spots the bidder would fill if every winning bid (including this one) is signed.
fn roster_spots_used(
    active_contract_count: usize,
    winning_bids: &[(i64, i16)],
    this_auction_id: i64,
) -> i32 {
    let other_winning_bids = winning_bids
        .iter()
        .filter(|(auction_id, _)| *auction_id != this_auction_id)
        .count();
    i32::try_from(active_contract_count + other_winning_bids + 1).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use fbkl_entity::auction::AuctionKind;

    use super::{
        BidRejection, committed_salary, requires_cap_and_roster_check, roster_spots_used,
        validate_bid_amount,
    };

    #[test]
    fn rebidding_on_your_own_winning_auction_swaps_the_amount() {
        let winning_bids = [(1, 10), (2, 20)];
        // re-bid on auction 2: 20 is replaced by 25, not added to it
        assert_eq!(committed_salary(100, &winning_bids, 2, 25), 135);
        // first bid on a different auction: everything counts
        assert_eq!(committed_salary(100, &winning_bids, 3, 25), 155);
        assert_eq!(roster_spots_used(20, &winning_bids, 2), 22);
        assert_eq!(roster_spots_used(20, &winning_bids, 3), 23);
    }

    #[test]
    fn in_season_free_agency_is_not_cap_gated() {
        assert!(!requires_cap_and_roster_check(
            AuctionKind::InSeasonFreeAgent
        ));
        assert!(requires_cap_and_roster_check(
            AuctionKind::PreseasonVeteranAuction
        ));
    }

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
