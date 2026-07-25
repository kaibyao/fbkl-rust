//! Player eligibility: which acquisition pool (veteran auction / rookie draft / neither) a player
//! belongs to, per spec 10.

mod classify;

pub use classify::{PlayerEligibilityFacts, classify_player};
