use std::fmt::Debug;

use chrono::{DateTime, FixedOffset};
use color_eyre::Result;
use fbkl_entity::{
    auction::{self, AuctionType},
    auction_queries, contract,
    sea_orm::ConnectionTrait,
};
use tracing::instrument;

/// Creates a new veteran auction for a given player + league.
#[instrument]
pub async fn start_new_auction_for_nba_player<C>(
    player_contract: &contract::Model,
    league_id: i64,
    end_of_season_year: i16,
    start_timestamp: DateTime<FixedOffset>,
    auction_type: AuctionType,
    starting_bid_amount: i16,
    db: &C,
) -> Result<auction::Model>
where
    C: ConnectionTrait + Debug,
{
    // Create the auction for it.
    let inserted_auction = auction_queries::insert_new_auction(
        player_contract.id,
        auction_type,
        starting_bid_amount,
        start_timestamp,
        None,
        db,
    )
    .await?;

    Ok(inserted_auction)
}
