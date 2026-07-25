//! Player eligibility: which acquisition pool (veteran auction / rookie draft / neither) a player
//! belongs to, per spec 10.

mod classify;
mod pools;

pub use classify::{PlayerEligibilityFacts, classify_player};
pub use pools::{
    VeteranAuctionPool, build_in_season_fa_pool, build_rookie_draft_eligible_pool,
    build_veteran_auction_pool,
};
