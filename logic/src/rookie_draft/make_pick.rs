//! Making a rookie-draft pick (§7.3) — the live counterpart of what `import-data` replays from CSV.
//!
//! Produces exactly the rows `seasonal_rookie_selection_transactions::process_rookie_selected`
//! produces (RD contract + `RookieDraftSelection` transaction + `AddViaRookieDraft` update),
//! but validates first: on the clock, in the eligible pool, not re-draft banned, roster room.

use std::{collections::HashSet, fmt::Debug};

use chrono::NaiveDate;
use color_eyre::{Result, eyre::eyre};
use fbkl_constants::league_rules::{
    PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT, rookie_draft_salary_for_round,
};
use fbkl_entity::{
    contract::{self, RelatedPlayer},
    contract_queries,
    deadline::DeadlineKind,
    deadline_queries, draft_pick_queries,
    rookie_draft_selection::{self, RookieDraftSelectionStatus},
    rookie_draft_selection_queries,
    sea_orm::{ActiveValue, ConnectionTrait, TransactionSession, TransactionTrait},
    team_update::{
        self, ContractUpdate, ContractUpdateType, TeamUpdateAsset, TeamUpdateData, TeamUpdateStatus,
    },
    team_update_queries::{self, ContractUpdatePlayerData},
    transaction_queries,
};
use tracing::instrument;

use crate::{
    eligibility::build_rookie_draft_eligible_pool,
    roster::{SalarySnapshot, calculate_team_contract_salary},
};

/// Why a pick was refused. Each variant maps to a distinct user-facing error code.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PickRejection {
    #[error("The draft has not started for this league season.")]
    DraftNotStarted,
    #[error("Selection {selection_id} is not on the clock (pick {on_the_clock_order} is).")]
    NotOnTheClock {
        selection_id: i64,
        on_the_clock_order: i16,
    },
    #[error("Selection {selection_id} has already been used or passed.")]
    SelectionAlreadyResolved { selection_id: i64 },
    #[error("Player is not in the rookie draft eligible pool (§7.5).")]
    PlayerNotEligible,
    #[error("Player was dropped during this draft and cannot be re-drafted (§7.3.4).")]
    ReDraftBanned,
    #[error("Team rosters {roster_used} contracts already, and the limit is {roster_limit}.")]
    NoRosterSpace {
        roster_used: usize,
        roster_limit: i16,
    },
}

/// The §7.3.4 re-draft ban set: players dropped during this draft, keyed the way a contract keys a
/// player (real NBA player or league-created one).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReDraftBan {
    players: HashSet<i64>,
    league_players: HashSet<i64>,
}

impl ReDraftBan {
    pub fn is_banned(&self, player_id: i64, is_league_player: bool) -> bool {
        if is_league_player {
            self.league_players.contains(&player_id)
        } else {
            self.players.contains(&player_id)
        }
    }

    fn from_dropped_contracts(dropped_contracts: &[contract::Model]) -> Self {
        let mut ban = Self::default();
        for dropped_contract in dropped_contracts {
            if let Some(player_id) = dropped_contract.player_id {
                ban.players.insert(player_id);
            }
            if let Some(league_player_id) = dropped_contract.league_player_id {
                ban.league_players.insert(league_player_id);
            }
        }
        ban
    }
}

/// The set of players banned from being (re-)drafted in this draft (§7.3.4).
///
/// Exposed on its own so the eligible-pool query can grey banned players out with a reason instead
/// of only failing at pick time.
#[instrument(skip(db))]
pub async fn re_draft_ban_check<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<ReDraftBan>
where
    C: ConnectionTrait,
{
    let dropped_contracts = rookie_draft_selection_queries::find_players_dropped_during_draft(
        league_id,
        end_of_season_year,
        db,
    )
    .await?;
    Ok(ReDraftBan::from_dropped_contracts(&dropped_contracts))
}

/// Drafts a player with the on-the-clock selection (§7.3).
///
/// `is_league_player` picks which id space `player_id` lives in, matching
/// `contract::Model::new_contract_from_rookie_draft`.
#[instrument(skip(db))]
pub async fn make_pick<C>(
    selection_id: i64,
    player_id: i64,
    is_league_player: bool,
    db: &C,
) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + TransactionTrait,
{
    let db_txn = db.begin().await?;

    // Locking the slate row first is what serializes two clients racing the same pick.
    let selection_model =
        rookie_draft_selection_queries::find_selection_by_id_for_update(selection_id, &db_txn)
            .await?;
    if selection_model.status != RookieDraftSelectionStatus::Unused {
        return Err(PickRejection::SelectionAlreadyResolved { selection_id }.into());
    }

    let draft_pick_model =
        draft_pick_queries::find_draft_pick_by_id(selection_model.draft_pick_id, &db_txn).await?;
    let league_id = selection_model.league_id;
    let end_of_season_year = draft_pick_model.end_of_season_year;

    assert_on_the_clock(&selection_model, end_of_season_year, &db_txn).await?;

    let eligible_pool =
        build_rookie_draft_eligible_pool(league_id, end_of_season_year, &db_txn).await?;
    if !pool_contains(&eligible_pool, player_id, is_league_player) {
        return Err(PickRejection::PlayerNotEligible.into());
    }

    if re_draft_ban_check(league_id, end_of_season_year, &db_txn)
        .await?
        .is_banned(player_id, is_league_player)
    {
        return Err(PickRejection::ReDraftBanned.into());
    }

    // §7.3.2 wants roster room, not cap (RD is off-cap); preseason draft = the 32-contract limit.
    let drafting_team_id = selection_model.current_owner_team_id;
    let active_contracts =
        contract_queries::find_active_contracts_for_team(drafting_team_id, &db_txn).await?;
    if active_contracts.len() >= usize::try_from(PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT)? {
        return Err(PickRejection::NoRosterSpace {
            roster_used: active_contracts.len(),
            roster_limit: PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT,
        }
        .into());
    }

    let deadline_model = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::PreseasonRookieDraftStart,
        &db_txn,
    )
    .await?;
    let SalarySnapshot {
        salary: previous_salary,
        cap: previous_salary_cap,
    } = calculate_team_contract_salary(
        drafting_team_id,
        &active_contracts,
        &deadline_model,
        &db_txn,
    )
    .await?;

    let rookie_contract_model = contract_queries::create_new_contract(
        contract::Model::new_contract_from_rookie_draft(
            league_id,
            end_of_season_year,
            drafting_team_id,
            rookie_draft_salary_for_round(draft_pick_model.round),
            player_id,
            is_league_player,
        ),
        &db_txn,
    )
    .await?;

    let mut updated_selection_model = rookie_draft_selection_queries::record_selection_result(
        selection_model,
        RookieDraftSelectionStatus::PlayerSelected,
        Some(rookie_contract_model.id),
        &db_txn,
    )
    .await?;

    let transaction_model = transaction_queries::insert_rookie_draft_selection_transaction(
        &deadline_model,
        updated_selection_model.id,
        &db_txn,
    )
    .await?;
    updated_selection_model.transaction_id = Some(transaction_model.id);

    insert_team_update_for_pick(
        &rookie_contract_model,
        &active_contracts,
        SalarySnapshot {
            salary: previous_salary,
            cap: previous_salary_cap,
        },
        deadline_model.date_time.date_naive(),
        transaction_model.id,
        &db_txn,
    )
    .await?;

    db_txn.commit().await?;

    Ok(updated_selection_model)
}

/// The `Done` `team_update` recording the drafted player joining the roster.
#[instrument(skip(db))]
async fn insert_team_update_for_pick<C>(
    rookie_contract_model: &contract::Model,
    active_contracts: &[contract::Model],
    salary_snapshot: SalarySnapshot,
    effective_date: NaiveDate,
    transaction_id: i64,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let contract_update_player_data =
        ContractUpdatePlayerData::from_contract_model(rookie_contract_model, db).await?;
    let mut team_contract_ids: Vec<i64> = active_contracts
        .iter()
        .map(|contract_model| contract_model.id)
        .collect();
    team_contract_ids.push(rookie_contract_model.id);
    let SalarySnapshot { salary, cap } = salary_snapshot;
    // Salary is unchanged either side of the pick: RD contracts do not count against the cap.
    let team_update_data = TeamUpdateData::from_assets(
        team_contract_ids,
        vec![TeamUpdateAsset::Contracts(vec![ContractUpdate {
            contract_id: rookie_contract_model.id,
            update_type: ContractUpdateType::AddViaRookieDraft,
            player_name_at_time: contract_update_player_data.player_name,
            player_team_abbr_at_time: contract_update_player_data.real_team_abbr,
            player_team_name_at_time: contract_update_player_data.real_team_name,
        }])],
        salary,
        cap,
        salary,
        cap,
    );
    team_update_queries::insert_team_update(
        team_update::ActiveModel {
            data: ActiveValue::Set(team_update_data.to_json()?),
            effective_date: ActiveValue::Set(effective_date),
            status: ActiveValue::Set(TeamUpdateStatus::Done),
            team_id: ActiveValue::Set(
                rookie_contract_model
                    .team_id
                    .ok_or_else(|| eyre!("Rookie draft contract has no team."))?,
            ),
            transaction_id: ActiveValue::Set(Some(transaction_id)),
            ..Default::default()
        },
        db,
    )
    .await?;
    Ok(())
}

/// Re-asserts inside the db transaction that this selection is still the lowest-`order` `Unused`
/// row, so a pick cannot jump the queue.
#[instrument(skip(db))]
pub(super) async fn assert_on_the_clock<C>(
    selection_model: &rookie_draft_selection::Model,
    end_of_season_year: i16,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait,
{
    let maybe_on_the_clock = rookie_draft_selection_queries::get_on_the_clock_selection(
        selection_model.league_id,
        end_of_season_year,
        db,
    )
    .await?;
    let Some(on_the_clock) = maybe_on_the_clock else {
        return Err(PickRejection::DraftNotStarted.into());
    };
    if on_the_clock.id != selection_model.id {
        return Err(PickRejection::NotOnTheClock {
            selection_id: selection_model.id,
            on_the_clock_order: on_the_clock.order,
        }
        .into());
    }
    Ok(())
}

fn pool_contains(pool: &[RelatedPlayer], player_id: i64, is_league_player: bool) -> bool {
    pool.iter().any(|related_player| match related_player {
        RelatedPlayer::LeaguePlayer(league_player_model) => {
            is_league_player && league_player_model.id == player_id
        }
        RelatedPlayer::Player(player_model) => !is_league_player && player_model.id == player_id,
    })
}

#[cfg(test)]
mod tests {
    use fbkl_constants::league_rules::rookie_draft_salary_for_round;
    use fbkl_entity::contract::{self, ContractKind, ContractStatus};

    use super::ReDraftBan;

    fn dropped_contract(
        maybe_player_id: Option<i64>,
        maybe_league_player_id: Option<i64>,
    ) -> contract::Model {
        contract::Model {
            id: 1,
            year_number: 1,
            kind: ContractKind::Veteran,
            is_ir: false,
            salary: 1,
            end_of_season_year: 2026,
            status: ContractStatus::Replaced,
            league_id: 1,
            league_player_id: maybe_league_player_id,
            player_id: maybe_player_id,
            previous_contract_id: None,
            original_contract_id: None,
            team_id: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn re_draft_ban_covers_both_player_id_spaces() {
        let ban = ReDraftBan::from_dropped_contracts(&[
            dropped_contract(Some(10), None),
            dropped_contract(None, Some(20)),
        ]);

        assert!(ban.is_banned(10, false));
        assert!(ban.is_banned(20, true));
        // Same number, wrong id space, must not collide.
        assert!(!ban.is_banned(10, true));
        assert!(!ban.is_banned(20, false));
        assert!(!ban.is_banned(99, false));
    }

    #[test]
    fn no_drops_bans_nobody() {
        let ban = ReDraftBan::from_dropped_contracts(&[]);
        assert!(!ban.is_banned(1, false));
        assert!(!ban.is_banned(1, true));
    }

    #[test]
    fn rd_salary_follows_the_pick_round() {
        let salaries: Vec<i16> = (1..=5).map(rookie_draft_salary_for_round).collect();
        assert_eq!(salaries, vec![4, 3, 2, 1, 1]);
    }
}
