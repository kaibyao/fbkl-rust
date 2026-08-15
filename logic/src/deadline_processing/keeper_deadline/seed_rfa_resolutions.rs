//! Records who owned each restricted free agent at the keeper deadline (rules §14.4, §15.4.2).
//!
//! The kind change itself is not done here. Annual advancement already turns a Rookie year 3 into
//! a `RestrictedFreeAgent`, a Veteran year 3 into a `UnrestrictedFreeAgentVeteran` and a
//! `RookieExtension` year 5 into a `UnrestrictedFreeAgentOriginalTeam`
//! (`entity/src/entities/contract/annual_contract_advancement.rs`), and that runs at
//! `PreseasonStart`, before the keeper deadline. The keeper deadline then leaves RFA/UFA contracts
//! alone: they can be neither kept nor dropped.
//!
//! What is missing is the snapshot. An RFA contract can be traded between the keeper deadline and
//! the close of its auction, and the discount in `sign_rfa_or_ufa_contract_to_team` keys off the
//! contract's `team_id`. So the owner at the deadline is copied into
//! `rfa_resolution.original_owner_team_id` while it is still correct.
//!
//! UFAs need no row: they have no raise/match handshake, and their discount right is read straight
//! off the contract they carry into the auction.

use color_eyre::Result;
use fbkl_entity::{
    contract::ContractKind,
    contract_queries,
    rfa_resolution::{self, RfaResolutionStatus},
    rfa_resolution_queries::{self, NewRfaResolution},
    sea_orm::ConnectionTrait,
};
use tracing::{instrument, warn};

/// Seeds one `rfa_resolution` row per designated RFA contract in the season.
///
/// Idempotent: a contract that already has a resolution is skipped, so re-processing the keeper
/// deadline cannot fork a player's resolution.
#[instrument(skip(db))]
pub async fn seed_rfa_resolutions<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<rfa_resolution::Model>>
where
    C: ConnectionTrait,
{
    let active_season_contracts = contract_queries::find_active_contracts_in_league_for_season(
        league_id,
        end_of_season_year,
        db,
    )
    .await?;

    let mut seeded_rfa_resolutions = vec![];
    for contract_model in active_season_contracts {
        if contract_model.kind != ContractKind::RestrictedFreeAgent {
            continue;
        }
        // An unowned RFA has nobody holding the §15.4.2 discount right; it just goes to the auction.
        let Some(original_owner_team_id) = contract_model.team_id else {
            warn!(
                contract_id = contract_model.id,
                "Skipping RFA resolution for a contract that is unowned at the keeper deadline."
            );
            continue;
        };
        if rfa_resolution_queries::find_rfa_resolution_for_contract(contract_model.id, db)
            .await?
            .is_some()
        {
            continue;
        }

        let seeded_rfa_resolution = rfa_resolution_queries::insert_rfa_resolution(
            NewRfaResolution {
                league_id,
                end_of_season_year,
                rfa_contract_id: contract_model.id,
                original_owner_team_id,
                auction_id: None,
                winning_team_id: None,
                final_bid: None,
                final_bid_at: None,
                status: RfaResolutionStatus::AwaitingAuction,
                raise_deadline_at: None,
            },
            db,
        )
        .await?;
        seeded_rfa_resolutions.push(seeded_rfa_resolution);
    }

    Ok(seeded_rfa_resolutions)
}
