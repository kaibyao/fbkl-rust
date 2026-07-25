//! Eligibility pool membership (spec 10 / rules §6.2, §7.5, §8.4).
//!
//! All three pools are the same two-step read: classify every candidate player, then subtract the
//! players a team currently rosters. Nothing keys off historical contract rows — §7.5.3 is explicit
//! that prior league draft/ownership never affects eligibility, so a previously-drafted,
//! now-unrostered, never-NBA player is still in the rookie draft pool.
//!
//! Membership only. Minimum bids and the auction schedule belong to spec 01.

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

use color_eyre::Result;
use fbkl_entity::{
    contract::{self, ContractKind, RelatedPlayer},
    contract_queries, league_player_queries,
    player::EligibilityClassification,
    player_queries,
    sea_orm::ConnectionTrait,
};
use tracing::instrument;

use super::{PlayerEligibilityFacts, classify_player};

/// Identifies a pool member the same way a contract does: a real NBA player or a league-created one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PlayerRef {
    Player(i64),
    LeaguePlayer(i64),
}

/// §6.2.2 — the veteran auction pool is not one flat list; RFAs are auctioned in the first week only
/// and their original owner may not bid, so consumers need the split.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VeteranAuctionPool {
    pub restricted_free_agents: Vec<RelatedPlayer>,
    pub unrestricted_free_agents: Vec<RelatedPlayer>,
    pub free_agents: Vec<RelatedPlayer>,
}

/// Which §6.2.2 bucket an unrostered player's current contract kind puts them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FreeAgencyBucket {
    Restricted,
    Unrestricted,
    FreeAgent,
}

/// No active contract means never-owned or long-expired, which is a plain free agent.
const fn free_agency_bucket(maybe_kind: Option<ContractKind>) -> FreeAgencyBucket {
    match maybe_kind {
        Some(ContractKind::RestrictedFreeAgent) => FreeAgencyBucket::Restricted,
        Some(
            ContractKind::UnrestrictedFreeAgentOriginalTeam
            | ContractKind::UnrestrictedFreeAgentVeteran,
        ) => FreeAgencyBucket::Unrestricted,
        _ => FreeAgencyBucket::FreeAgent,
    }
}

/// The league's current roster + free-agency picture for one season.
#[derive(Debug, Default)]
struct RosterSnapshot {
    /// Players a team currently holds — keepers and already-acquired players.
    rostered: HashSet<PlayerRef>,
    /// Contract kind of unrostered players, which drives the §6.2.2 partition.
    free_agent_kinds: HashMap<PlayerRef, ContractKind>,
}

impl RosterSnapshot {
    /// A contract can name a real player, a league player, or (after a league player is linked to an
    /// NBA entry) effectively both, so every id present is recorded.
    fn from_contracts(contracts: &[contract::Model]) -> Self {
        let mut snapshot = Self::default();
        for contract_model in contracts {
            let refs = [
                contract_model.player_id.map(PlayerRef::Player),
                contract_model.league_player_id.map(PlayerRef::LeaguePlayer),
            ];
            for player_ref in refs.into_iter().flatten() {
                if contract_model.team_id.is_some() {
                    snapshot.rostered.insert(player_ref);
                } else {
                    snapshot
                        .free_agent_kinds
                        .insert(player_ref, contract_model.kind);
                }
            }
        }
        snapshot
    }
}

/// Candidate players classified into one of `allowed` and not currently rostered.
#[instrument(skip(db))]
async fn build_pool<C>(
    league_id: i64,
    end_of_season_year: i16,
    allowed: &[EligibilityClassification],
    db: &C,
) -> Result<(Vec<(PlayerRef, RelatedPlayer)>, RosterSnapshot)>
where
    C: ConnectionTrait + Debug,
{
    let contracts = contract_queries::find_active_contracts_in_league_for_season(
        league_id,
        end_of_season_year,
        db,
    )
    .await?;
    let snapshot = RosterSnapshot::from_contracts(&contracts);

    let players = player_queries::find_eligibility_candidate_players(db).await?;
    let league_players =
        league_player_queries::find_league_players_in_league(league_id, db).await?;

    let candidates = players
        .into_iter()
        .map(|model| {
            (
                PlayerRef::Player(model.id),
                PlayerEligibilityFacts::from(&model),
                RelatedPlayer::Player(model),
            )
        })
        // A linked league player duplicates its real-player row; the real row wins.
        .chain(
            league_players
                .into_iter()
                .filter(|model| model.real_player_id.is_none())
                .map(|model| {
                    (
                        PlayerRef::LeaguePlayer(model.id),
                        PlayerEligibilityFacts::from(&model),
                        RelatedPlayer::LeaguePlayer(model),
                    )
                }),
        );

    let members = candidates
        .filter(|(player_ref, facts, _)| {
            allowed.contains(&classify_player(*facts)) && !snapshot.rostered.contains(player_ref)
        })
        .map(|(player_ref, _, related_player)| (player_ref, related_player))
        .collect();

    Ok((members, snapshot))
}

/// §6.2.1 — every player who has been on an active NBA roster and is not a keeper, split per §6.2.2.
#[instrument(skip(db))]
pub async fn build_veteran_auction_pool<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<VeteranAuctionPool>
where
    C: ConnectionTrait + Debug,
{
    let (members, snapshot) = build_pool(
        league_id,
        end_of_season_year,
        &[EligibilityClassification::VeteranAuctionEligible],
        db,
    )
    .await?;

    let mut pool = VeteranAuctionPool::default();
    for (player_ref, related_player) in members {
        let bucket = free_agency_bucket(snapshot.free_agent_kinds.get(&player_ref).copied());
        match bucket {
            FreeAgencyBucket::Restricted => pool.restricted_free_agents.push(related_player),
            FreeAgencyBucket::Unrestricted => pool.unrestricted_free_agents.push(related_player),
            FreeAgencyBucket::FreeAgent => pool.free_agents.push(related_player),
        }
    }
    Ok(pool)
}

/// §7.5 — never-NBA players in the draft-eligible set who are not currently rostered.
#[instrument(skip(db))]
pub async fn build_rookie_draft_eligible_pool<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<RelatedPlayer>>
where
    C: ConnectionTrait + Debug,
{
    let (members, _) = build_pool(
        league_id,
        end_of_season_year,
        &[EligibilityClassification::RookieDraftEligible],
        db,
    )
    .await?;
    Ok(members.into_iter().map(|(_, player)| player).collect())
}

/// §8.4 — the union of the auction and draft pools minus currently-rostered players. `Ineligible`
/// players stay out (§8.4.2).
#[instrument(skip(db))]
pub async fn build_in_season_fa_pool<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<RelatedPlayer>>
where
    C: ConnectionTrait + Debug,
{
    let (members, _) = build_pool(
        league_id,
        end_of_season_year,
        &[
            EligibilityClassification::VeteranAuctionEligible,
            EligibilityClassification::RookieDraftEligible,
        ],
        db,
    )
    .await?;
    Ok(members.into_iter().map(|(_, player)| player).collect())
}

#[cfg(test)]
mod tests {
    use fbkl_entity::contract::{ContractKind, ContractStatus};

    use super::{FreeAgencyBucket, PlayerRef, RosterSnapshot, free_agency_bucket};

    fn contract(
        player_id: i64,
        team_id: Option<i64>,
        kind: ContractKind,
    ) -> fbkl_entity::contract::Model {
        fbkl_entity::contract::Model {
            id: player_id,
            year_number: 1,
            kind,
            is_ir: false,
            salary: 10,
            end_of_season_year: 2025,
            status: ContractStatus::Active,
            league_id: 1,
            league_player_id: None,
            player_id: Some(player_id),
            previous_contract_id: None,
            original_contract_id: Some(player_id),
            team_id,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn snapshot_separates_rostered_from_free_agents() {
        let snapshot = RosterSnapshot::from_contracts(&[
            contract(1, Some(7), ContractKind::Veteran),
            contract(2, None, ContractKind::RestrictedFreeAgent),
        ]);

        assert!(snapshot.rostered.contains(&PlayerRef::Player(1)));
        assert!(!snapshot.rostered.contains(&PlayerRef::Player(2)));
        assert_eq!(
            snapshot.free_agent_kinds.get(&PlayerRef::Player(2)),
            Some(&ContractKind::RestrictedFreeAgent)
        );
        assert!(
            !snapshot
                .free_agent_kinds
                .contains_key(&PlayerRef::Player(1))
        );
    }

    #[test]
    fn free_agency_buckets_follow_contract_kind() {
        assert_eq!(
            free_agency_bucket(Some(ContractKind::RestrictedFreeAgent)),
            FreeAgencyBucket::Restricted
        );
        assert_eq!(
            free_agency_bucket(Some(ContractKind::UnrestrictedFreeAgentOriginalTeam)),
            FreeAgencyBucket::Unrestricted
        );
        assert_eq!(
            free_agency_bucket(Some(ContractKind::UnrestrictedFreeAgentVeteran)),
            FreeAgencyBucket::Unrestricted
        );
        assert_eq!(
            free_agency_bucket(Some(ContractKind::FreeAgent)),
            FreeAgencyBucket::FreeAgent
        );
        assert_eq!(free_agency_bucket(None), FreeAgencyBucket::FreeAgent);
    }
}
