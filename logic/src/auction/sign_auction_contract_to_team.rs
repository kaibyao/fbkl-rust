use std::fmt::Debug;

use color_eyre::Result;
use fbkl_entity::{
    auction, auction_bid, contract, contract_queries, deadline,
    sea_orm::{ConnectionTrait, TransactionTrait},
    team_update, team_update_queries, transaction, transaction_queries,
};
use tracing::instrument;

/// Signs a contract to the team that submitted the last/winning bid to an auction before it ended. Creates + inserts the contract, transaction, and team update.
#[instrument]
pub async fn sign_auction_contract_to_team<C>(
    auction_model: &auction::Model,
    winning_auction_bid_model: &auction_bid::Model,
    deadline_model: &deadline::Model,
    db: &C,
) -> Result<(contract::Model, transaction::Model, team_update::Model)>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    // Sign contract to team
    let signed_contract_model = contract_queries::sign_auction_contract_to_team(
        auction_model,
        winning_auction_bid_model,
        db,
    )
    .await?;

    // Create transaction
    let auction_transaction_model =
        transaction_queries::insert_auction_transaction(deadline_model, auction_model.id, db)
            .await?;

    // Create team_update
    let team_update_model = team_update_queries::insert_team_update_from_auction_won(
        auction_model,
        winning_auction_bid_model,
        &auction_transaction_model,
        &signed_contract_model,
        db,
    )
    .await?;

    Ok((
        signed_contract_model,
        auction_transaction_model,
        team_update_model,
    ))
}
