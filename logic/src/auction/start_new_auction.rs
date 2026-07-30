use std::fmt::Debug;

use chrono::{DateTime, FixedOffset};
use color_eyre::Result;
use fbkl_entity::{
    auction::{self, AuctionKind},
    auction_queries::{self, NewAuction},
    contract,
    sea_orm::ConnectionTrait,
};
use tracing::instrument;

use super::{auction_close_at, auction_quiet_window};

/// Creates a new veteran auction for a given player + league.
#[instrument]
pub async fn start_new_auction_for_nba_player<C>(
    player_contract: &contract::Model,
    league_id: i64,
    end_of_season_year: i16,
    start_timestamp: DateTime<FixedOffset>,
    auction_type: AuctionKind,
    starting_bid_amount: i16,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    // Historical replay opens and closes an auction in one go, so the quiet window is all it needs.
    let close_at_timestamp = auction_close_at(
        start_timestamp,
        auction_quiet_window(start_timestamp, None),
        None,
        None,
    )?;
    let inserted_auction = auction_queries::insert_new_auction(
        NewAuction {
            contract_id: player_contract.id,
            kind: auction_type,
            minimum_bid_amount: starting_bid_amount,
            start_timestamp,
            close_at_timestamp,
            all_bid_deadline_timestamp: None,
            original_owner_team_id: None,
        },
        db,
    )
    .await?;

    Ok(inserted_auction)
}
