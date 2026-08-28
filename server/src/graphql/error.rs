//! Stable GraphQL error codes.
//!
//! Every error returned from a resolver carries a machine-readable `code`
//! extension so the frontend can switch on the failure instead of parsing
//! message strings. Domain-validation failures (keeper limits, chain staleness,
//! not-on-the-clock) get their own code and never surface as a bare 500.

use async_graphql::{Error as GraphQlError, ErrorExtensions};
use axum::http::StatusCode;

use crate::error::FbklError;

/// Machine-readable value of the `code` error extension.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    /// Not logged in.
    Unauthenticated,
    /// Logged in, but not allowed to do this.
    Forbidden,
    /// Asset does not exist (or is not visible in the selected league).
    NotFound,
    /// Client sent an unusable argument or has no league selected.
    BadRequest,
    /// Acting on a trade/contract that a newer record in its chain has superseded.
    NotLatestInChain,
    /// A team involved in a trade has no pre-trade salary snapshot, so it cannot be processed.
    MissingPreTradeSalary,
    /// Submitted keepers break a league keeper rule (ineligible kind, count limit, salary limit).
    KeeperValidationFailed,
    /// The auction is not taking bids (wrong status, or its bidding window elapsed).
    AuctionNotOpen,
    /// Bid is under the auction's minimum bid.
    BidBelowMinimum,
    /// Bid does not clear the current high bid by the required $1 increment.
    BidBelowIncrement,
    /// Winning the bid would put the bidding team over its salary cap (rules §6.4.1.1).
    BidInsufficientCap,
    /// Winning the bid would leave the bidding team without an open roster spot (rules §6.4.1.2).
    BidNoRosterSpace,
    /// The player's previous team may not bid on that player's own auction (rules §6.2.2.3).
    BidOriginalOwner,
    /// A bid on an RFA must name the pick it would forfeit (rules §15.3.3).
    BidMissingCompensationPick,
    /// The named pick cannot settle what the bid would owe, or the auction owes nothing (rules §15.3.3).
    BidIneligibleCompensationPick,
    /// Season config (tiers, ranked list) is locked because the veteran auction pool is assembled (rules §6.3.6).
    VeteranAuctionStarted,
    /// The rookie draft has not been started for this league season.
    DraftNotStarted,
    /// The referenced selection is not the one on the clock.
    DraftNotOnTheClock,
    /// The referenced selection was already used or passed.
    DraftSelectionResolved,
    /// The player is not in the rookie draft eligible pool (rules §7.5).
    DraftPlayerNotEligible,
    /// The player was dropped during this draft and cannot be re-drafted (rules §7.3.4).
    DraftReDraftBanned,
    /// Drafting would leave the picking team over its roster limit (rules §7.3.2).
    DraftNoRosterSpace,
    /// The season's lottery has already been drawn and cannot be re-rolled (rules §7.2.5).
    DraftLotteryAlreadyRun,
    /// The team's roster breaks a roster-lock rule (cap, roster limits, IR slots).
    RosterIllegal,
    /// A league rule refuses this single roster move (IR eligibility, same-week-add drop, contract kind).
    RosterMoveRejected,
    /// Server-side fault; message is deliberately generic.
    Internal,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthenticated => "UNAUTHENTICATED",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::BadRequest => "BAD_REQUEST",
            Self::NotLatestInChain => "NOT_LATEST_IN_CHAIN",
            Self::MissingPreTradeSalary => "MISSING_PRE_TRADE_SALARY",
            Self::KeeperValidationFailed => "KEEPER_VALIDATION_FAILED",
            Self::AuctionNotOpen => "AUCTION_NOT_OPEN",
            Self::BidBelowMinimum => "BID_BELOW_MINIMUM",
            Self::BidBelowIncrement => "BID_BELOW_INCREMENT",
            Self::BidInsufficientCap => "BID_INSUFFICIENT_CAP",
            Self::BidNoRosterSpace => "BID_NO_ROSTER_SPACE",
            Self::BidOriginalOwner => "BID_ORIGINAL_OWNER",
            Self::BidMissingCompensationPick => "BID_MISSING_COMPENSATION_PICK",
            Self::BidIneligibleCompensationPick => "BID_INELIGIBLE_COMPENSATION_PICK",
            Self::VeteranAuctionStarted => "VETERAN_AUCTION_STARTED",
            Self::DraftNotStarted => "DRAFT_NOT_STARTED",
            Self::DraftNotOnTheClock => "DRAFT_NOT_ON_THE_CLOCK",
            Self::DraftSelectionResolved => "DRAFT_SELECTION_RESOLVED",
            Self::DraftPlayerNotEligible => "DRAFT_PLAYER_NOT_ELIGIBLE",
            Self::DraftReDraftBanned => "DRAFT_RE_DRAFT_BANNED",
            Self::DraftNoRosterSpace => "DRAFT_NO_ROSTER_SPACE",
            Self::DraftLotteryAlreadyRun => "DRAFT_LOTTERY_ALREADY_RUN",
            Self::RosterIllegal => "ROSTER_ILLEGAL",
            Self::RosterMoveRejected => "ROSTER_MOVE_REJECTED",
            Self::Internal => "INTERNAL",
        }
    }

    const fn default_message(self) -> &'static str {
        match self {
            Self::Unauthenticated => "not logged in",
            Self::Forbidden => "not allowed",
            Self::NotFound => "not found",
            Self::BadRequest => "bad request",
            Self::NotLatestInChain => "a newer version of this record supersedes it",
            Self::MissingPreTradeSalary => {
                "a team involved in this trade is missing its pre-trade salary"
            }
            Self::KeeperValidationFailed => "these keepers break a league keeper rule",
            Self::AuctionNotOpen => "this auction is not taking bids",
            Self::BidBelowMinimum => "bid is below the auction's minimum bid",
            Self::BidBelowIncrement => "bid does not clear the current high bid",
            Self::BidInsufficientCap => "bid would exceed your salary cap",
            Self::BidNoRosterSpace => "bid would exceed your roster limit",
            Self::BidOriginalOwner => "you cannot bid on your own former player's auction",
            Self::BidMissingCompensationPick => "bid must name the draft pick it would forfeit",
            Self::BidIneligibleCompensationPick => {
                "that draft pick cannot settle what the bid would owe"
            }
            Self::VeteranAuctionStarted => {
                "the veteran auction has started, so this season's config is locked"
            }
            Self::DraftNotStarted => "the rookie draft has not started",
            Self::DraftNotOnTheClock => "that pick is not on the clock",
            Self::DraftSelectionResolved => "that pick has already been used or passed",
            Self::DraftPlayerNotEligible => "that player is not eligible for the rookie draft",
            Self::DraftReDraftBanned => "that player was dropped during this draft",
            Self::DraftNoRosterSpace => "drafting would exceed your roster limit",
            Self::DraftLotteryAlreadyRun => "this season's lottery has already been drawn",
            Self::RosterIllegal => "this roster breaks a roster rule",
            Self::RosterMoveRejected => "a league rule does not allow this roster move",
            Self::Internal => "internal server error",
        }
    }
}

/// Build a GraphQL error carrying `code` plus a caller-supplied message.
pub fn graphql_error(code: ErrorCode, message: impl Into<String>) -> GraphQlError {
    GraphQlError::new(message).extend_with(|_, ext| ext.set("code", code.as_str()))
}

/// Build a GraphQL error carrying `code` and its generic message.
pub fn code_error(code: ErrorCode) -> GraphQlError {
    graphql_error(code, code.default_message())
}

/// Convert an `FbklError` into a coded GraphQL error, logging server faults.
pub fn from_fbkl(error: &FbklError) -> GraphQlError {
    let status = error.status_code();
    let code = match status {
        StatusCode::UNAUTHORIZED => ErrorCode::Unauthenticated,
        StatusCode::FORBIDDEN => ErrorCode::Forbidden,
        StatusCode::NOT_FOUND => ErrorCode::NotFound,
        _ if status.is_client_error() => ErrorCode::BadRequest,
        _ => ErrorCode::Internal,
    };

    if code == ErrorCode::Internal {
        tracing::error!(error = ?error, "graphql resolver failed");
        return code_error(code);
    }

    graphql_error(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_attached_as_an_extension() {
        let error = code_error(ErrorCode::Forbidden);
        let code = error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .cloned();

        assert_eq!(code, Some("FORBIDDEN".into()));
        assert_eq!(error.message, "not allowed");
    }

    #[test]
    fn fbkl_server_faults_are_coded_internal_and_not_leaked() {
        let error = from_fbkl(&FbklError::from(StatusCode::INTERNAL_SERVER_ERROR));

        assert_eq!(error.message, "internal server error");
        assert_eq!(
            error.extensions.as_ref().and_then(|ext| ext.get("code")),
            Some(&"INTERNAL".into())
        );
    }

    #[test]
    fn fbkl_client_faults_keep_their_message() {
        let error = from_fbkl(&FbklError::BadRequest("bad team id".to_owned()));

        assert_eq!(error.message, "bad team id");
        assert_eq!(
            error.extensions.as_ref().and_then(|ext| ext.get("code")),
            Some(&"BAD_REQUEST".into())
        );
    }
}
