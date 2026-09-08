//! The rookie draft surface (spec 02, rules §7): board reads, the audited lottery, the eligible
//! pool, and the make/pass/lottery/start/standings mutations.
//!
//! Every rule lives in `fbkl_logic::rookie_draft`; these resolvers only authorize, fetch and map.
//! Pick ownership is re-derived from the stored on-the-clock selection, never from a client id.

use std::collections::HashMap;

use async_graphql::{Context, Error as GraphQlError, InputObject, Object, Result, SimpleObject};
use color_eyre::Report;
use fbkl_constants::league_rules::{DRAFT_PICK_ROUNDS, rookie_draft_salary_for_round};
use fbkl_entity::{
    contract::{self, RelatedPlayer},
    contract_queries::{find_contract_by_id, find_contracts_by_ids},
    draft_pick,
    draft_pick_queries::{find_draft_pick_by_id, get_draft_picks_for_league_season},
    league_team_season_standing_queries::{
        NewLeagueTeamSeasonStanding, find_standings_for_league_season,
        upsert_standings_for_league_season,
    },
    rookie_draft_lottery_queries::{find_lottery_for_league_season, find_lottery_picks},
    rookie_draft_selection::{self, RookieDraftSelectionStatus},
    rookie_draft_selection_queries::{get_on_the_clock_selection, get_selections_for_draft},
    sea_orm::DatabaseConnection,
};
use fbkl_logic::{
    eligibility::build_rookie_draft_eligible_pool,
    rookie_draft::{
        PickRejection, make_pick, pass_pick, re_draft_ban_check, run_lottery, start_rookie_draft,
    },
};

use crate::graphql::{
    ErrorCode, LeagueRoleGuard, RoleRequirement, code_error, contract::Contract, current_season,
    graphql_error, player::LeagueOrRealPlayer, require_league_role,
};

/// The eligible pool spans every rookie-eligible player, so a page is always bounded (same
/// convention as the league event feed and auction bid history).
const MAX_PAGE_SIZE: usize = 100;

#[derive(SimpleObject)]
pub struct DraftPick {
    pub id: i64,
    pub round: i16,
    pub end_of_season_year: i16,
    pub league_id: i64,
    pub current_owner_team_id: i64,
    pub original_owner_team_id: i64,
}

impl DraftPick {
    pub const fn from_model(model: &draft_pick::Model) -> Self {
        Self {
            id: model.id,
            round: model.round,
            end_of_season_year: model.end_of_season_year,
            league_id: model.league_id,
            current_owner_team_id: model.current_owner_team_id,
            original_owner_team_id: model.original_owner_team_id,
        }
    }
}

/// One slate slot. `contract` is the signed rookie contract when a player was selected — its
/// `leagueOrRealPlayer` field resolves through the player `DataLoader`s, so a 60-cell board is
/// still two player queries.
#[derive(SimpleObject)]
pub struct RookieDraftSelection {
    pub id: i64,
    pub order: i16,
    pub status: RookieDraftSelectionStatus,
    pub contract_id: Option<i64>,
    pub draft_pick_id: i64,
    pub current_owner_team_id: i64,
    pub round: Option<i16>,
    /// The fixed RD salary this slot signs at (§7.4.1). Null when the round is unknown.
    pub salary: Option<i16>,
    pub contract: Option<Contract>,
}

impl RookieDraftSelection {
    fn from_model(
        model: &rookie_draft_selection::Model,
        round: Option<i16>,
        contract_model: Option<&contract::Model>,
    ) -> Result<Self> {
        let contract = contract_model
            .map(Contract::from_model)
            .transpose()
            .map_err(|err| {
                tracing::error!(error = ?err, "rookie draft contract has no player");
                code_error(ErrorCode::Internal)
            })?;

        Ok(Self {
            id: model.id,
            order: model.order,
            status: model.status,
            contract_id: model.contract_id,
            draft_pick_id: model.draft_pick_id,
            current_owner_team_id: model.current_owner_team_id,
            round,
            salary: round.and_then(salary_for_round),
            contract,
        })
    }
}

/// A season's draft: every pick, the slate, and who is on the clock.
#[derive(SimpleObject)]
pub struct DraftBoard {
    pub end_of_season_year: i16,
    pub picks: Vec<DraftPick>,
    pub selections: Vec<RookieDraftSelection>,
    /// False until `startRookieDraft` has built the slate.
    pub started: bool,
    pub on_the_clock_selection_id: Option<i64>,
    pub on_the_clock_team_id: Option<i64>,
}

#[derive(SimpleObject)]
pub struct RookieDraftLotteryPick {
    pub pick_number: i16,
    pub team_id: i64,
    pub balls_held: i16,
}

/// The audited lottery: the committed seed plus the drawn slots, so the draw can be re-verified.
#[derive(SimpleObject)]
pub struct RookieDraftLottery {
    pub end_of_season_year: i16,
    pub rng_seed: i64,
    pub rng_log: Option<String>,
    pub picks: Vec<RookieDraftLotteryPick>,
}

/// A pool entry the UI can show greyed out with a reason instead of failing at pick time.
#[derive(SimpleObject)]
pub struct EligibleRookie {
    pub player: LeagueOrRealPlayer,
    pub banned: bool,
    pub banned_reason: Option<String>,
}

/// One page of the eligible pool.
#[derive(SimpleObject)]
pub struct PagedEligibleRookies {
    pub items: Vec<EligibleRookie>,
    pub total_items: u64,
}

/// One team's frozen standings inputs (§7.2), entered by the commissioner.
#[derive(InputObject)]
pub struct LeagueTeamSeasonStandingInput {
    pub team_id: i64,
    /// Final regular-season rank, 1 = best.
    pub regular_season_rank: i16,
    /// The ~2/3-season snapshot that drives lottery odds and the rounds 2-5 order.
    pub mid_season_rank: i16,
    pub made_playoffs: bool,
    /// 1 = champion .. 6 = first-round loser; null for non-playoff teams.
    pub playoff_finish: Option<i16>,
}

#[derive(Default)]
pub struct DraftQuery;

#[Object]
impl DraftQuery {
    /// The season's slate with status, on-the-clock team, and each pick's player and salary.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn draft_board(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<DraftBoard> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (league_id, season) = league_and_season(ctx, end_of_season_year).await?;

        let picks = get_draft_picks_for_league_season(league_id, season, db)
            .await
            .map_err(|err| internal("failed to load draft picks", &err))?;
        let selections = get_selections_for_draft(league_id, season, db)
            .await
            .map_err(|err| internal("failed to load rookie draft selections", &err))?;

        let rounds: HashMap<i64, i16> = picks.iter().map(|pick| (pick.id, pick.round)).collect();
        let contract_ids = selections.iter().filter_map(|s| s.contract_id).collect();
        let contracts: HashMap<i64, contract::Model> = find_contracts_by_ids(contract_ids, db)
            .await
            .map_err(|err| internal("failed to load rookie draft contracts", &err))?
            .into_iter()
            .map(|model| (model.id, model))
            .collect();

        let on_the_clock = selections
            .iter()
            .filter(|s| s.status == RookieDraftSelectionStatus::Unused)
            .min_by_key(|s| s.order);

        Ok(DraftBoard {
            end_of_season_year: season,
            picks: picks.iter().map(DraftPick::from_model).collect(),
            started: !selections.is_empty(),
            on_the_clock_selection_id: on_the_clock.map(|s| s.id),
            on_the_clock_team_id: on_the_clock.map(|s| s.current_owner_team_id),
            selections: selections
                .iter()
                .map(|model| {
                    RookieDraftSelection::from_model(
                        model,
                        rounds.get(&model.draft_pick_id).copied(),
                        model.contract_id.and_then(|id| contracts.get(&id)),
                    )
                })
                .collect::<Result<_>>()?,
        })
    }

    /// The season's picks ordered by round, then pick id. Defaults to the current season.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn draft_order(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Vec<DraftPick>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (league_id, season) = league_and_season(ctx, end_of_season_year).await?;

        let picks = get_draft_picks_for_league_season(league_id, season, db)
            .await
            .map_err(|err| internal("failed to load draft picks", &err))?;

        Ok(picks.iter().map(DraftPick::from_model).collect())
    }

    /// The audited lottery result, or null before the lottery has been drawn.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn rookie_draft_lottery(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
    ) -> Result<Option<RookieDraftLottery>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (league_id, season) = league_and_season(ctx, end_of_season_year).await?;

        let Some(lottery) = find_lottery_for_league_season(league_id, season, db)
            .await
            .map_err(|err| internal("failed to load the rookie draft lottery", &err))?
        else {
            return Ok(None);
        };

        let picks = find_lottery_picks(lottery.id, db)
            .await
            .map_err(|err| internal("failed to load the lottery picks", &err))?;

        Ok(Some(RookieDraftLottery {
            end_of_season_year: lottery.end_of_season_year,
            rng_seed: lottery.rng_seed,
            rng_log: lottery.rng_log,
            picks: picks
                .into_iter()
                .map(|pick| RookieDraftLotteryPick {
                    pick_number: pick.pick_number,
                    team_id: pick.team_id,
                    balls_held: pick.balls_held,
                })
                .collect(),
        }))
    }

    /// One page of the §7.5 eligible pool, flagging players who cannot be drafted and why.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn eligible_rookie_pool(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: Option<i16>,
        #[graphql(default = 0)] page: usize,
        #[graphql(default = 25)] page_size: usize,
    ) -> Result<PagedEligibleRookies> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (league_id, season) = league_and_season(ctx, end_of_season_year).await?;

        let pool = build_rookie_draft_eligible_pool(league_id, season, db)
            .await
            .map_err(|err| internal("failed to build the rookie draft pool", &err))?;
        let ban = re_draft_ban_check(league_id, season, db)
            .await
            .map_err(|err| internal("failed to check the re-draft ban", &err))?;
        let drafted = drafted_player_keys(league_id, season, db).await?;

        // The pool is a computed list rather than a query, so the page is a slice of it.
        let total_items = pool.len() as u64;
        let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
        let items = pool
            .into_iter()
            .skip(page.saturating_mul(page_size))
            .take(page_size)
            .map(|related_player| {
                let key = player_key(&related_player);
                let banned_reason = if drafted.contains(&key) {
                    Some("Already drafted in this draft.".to_owned())
                } else if ban.is_banned(key.0, key.1) {
                    Some("Dropped during this draft and cannot be re-drafted (§7.3.4).".to_owned())
                } else {
                    None
                };

                EligibleRookie {
                    player: LeagueOrRealPlayer::from_related_player(related_player, season),
                    banned: banned_reason.is_some(),
                    banned_reason,
                }
            })
            .collect();

        Ok(PagedEligibleRookies { items, total_items })
    }
}

#[derive(Default)]
pub struct DraftMutation;

#[Object]
impl DraftMutation {
    /// Ingests the frozen standings inputs the draft order and lottery odds derive from (§7.2).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn save_league_team_season_standings(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: i16,
        standings: Vec<LeagueTeamSeasonStandingInput>,
    ) -> Result<i32> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;

        let rows: Vec<NewLeagueTeamSeasonStanding> = standings
            .into_iter()
            .map(|row| NewLeagueTeamSeasonStanding {
                team_id: row.team_id,
                regular_season_rank: row.regular_season_rank,
                mid_season_rank: row.mid_season_rank,
                made_playoffs: row.made_playoffs,
                playoff_finish: row.playoff_finish,
            })
            .collect();
        let saved = i32::try_from(rows.len())
            .map_err(|_| graphql_error(ErrorCode::BadRequest, "too many standings rows"))?;

        upsert_standings_for_league_season(caller_team.league_id, end_of_season_year, rows, db)
            .await
            .map_err(|err| internal("failed to save the season standings", &err))?;

        Ok(saved)
    }

    /// Draws the first-round lottery (§7.2.5), returning the six team ids in drawn order.
    /// Audit detail (seed, ball counts) comes back from the `rookieDraftLottery` query.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn run_rookie_draft_lottery(
        &self,
        ctx: &Context<'_>,
        end_of_season_year: i16,
    ) -> Result<Vec<i64>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;
        let league_id = caller_team.league_id;

        if find_lottery_for_league_season(league_id, end_of_season_year, db)
            .await
            .map_err(|err| internal("failed to check for an existing lottery", &err))?
            .is_some()
        {
            return Err(code_error(ErrorCode::DraftLotteryAlreadyRun));
        }

        let standings = find_standings_for_league_season(league_id, end_of_season_year, db)
            .await
            .map_err(|err| internal("failed to load the season standings", &err))?;

        run_lottery(league_id, end_of_season_year, &standings, db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to run the rookie draft lottery");
                graphql_error(ErrorCode::BadRequest, err.to_string())
            })
    }

    /// Runs the lottery if needed, computes the order and builds the slate. False = already started.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Commissioner)")]
    async fn start_rookie_draft(&self, ctx: &Context<'_>, end_of_season_year: i16) -> Result<bool> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let (_, caller_team) = require_league_role(ctx, RoleRequirement::Commissioner).await?;

        start_rookie_draft(caller_team.league_id, end_of_season_year, db)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to start the rookie draft");
                graphql_error(ErrorCode::BadRequest, err.to_string())
            })
    }

    /// Drafts a player with the caller's own on-the-clock pick (§7.3).
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn make_rookie_draft_pick(
        &self,
        ctx: &Context<'_>,
        selection_id: i64,
        player_id: i64,
        #[graphql(default = false)] is_league_player: bool,
    ) -> Result<RookieDraftSelection> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        require_own_pick_on_the_clock(ctx, selection_id).await?;

        let selection = make_pick(selection_id, player_id, is_league_player, db)
            .await
            .map_err(|err| pick_error(&err))?;

        selection_view(&selection, db).await
    }

    /// Passes the caller's own on-the-clock pick (§7.3.1). The slot stays consumed.
    #[graphql(guard = "LeagueRoleGuard(RoleRequirement::Member)")]
    async fn pass_rookie_draft_pick(
        &self,
        ctx: &Context<'_>,
        selection_id: i64,
    ) -> Result<RookieDraftSelection> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        require_own_pick_on_the_clock(ctx, selection_id).await?;

        let selection = pass_pick(selection_id, db)
            .await
            .map_err(|err| pick_error(&err))?;

        selection_view(&selection, db).await
    }
}

/// Rejects anyone but the current owner of the pick that is actually on the clock. The owning team
/// comes from the stored selection row, so a client-supplied team id can never widen access.
async fn require_own_pick_on_the_clock(ctx: &Context<'_>, selection_id: i64) -> Result<()> {
    let db = ctx.data_unchecked::<DatabaseConnection>();
    let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
    let season = current_season(ctx, caller_team.league_id).await?;

    let on_the_clock = get_on_the_clock_selection(caller_team.league_id, season, db)
        .await
        .map_err(|err| internal("failed to load the on-the-clock selection", &err))?
        .ok_or_else(|| code_error(ErrorCode::DraftNotStarted))?;

    if on_the_clock.id != selection_id {
        return Err(code_error(ErrorCode::DraftNotOnTheClock));
    }
    if on_the_clock.current_owner_team_id != caller_team.id {
        return Err(code_error(ErrorCode::Forbidden));
    }

    Ok(())
}

/// A refused pick is the client's fault and gets its own code; anything else is a server fault.
fn pick_error(error: &Report) -> GraphQlError {
    let Some(rejection) = error.downcast_ref::<PickRejection>() else {
        tracing::error!(error = ?error, "failed to resolve a rookie draft pick");
        return code_error(ErrorCode::Internal);
    };

    let code = match rejection {
        PickRejection::DraftNotStarted => ErrorCode::DraftNotStarted,
        PickRejection::NotOnTheClock { .. } => ErrorCode::DraftNotOnTheClock,
        PickRejection::SelectionAlreadyResolved { .. } => ErrorCode::DraftSelectionResolved,
        PickRejection::PlayerNotEligible => ErrorCode::DraftPlayerNotEligible,
        PickRejection::ReDraftBanned => ErrorCode::DraftReDraftBanned,
        PickRejection::NoRosterSpace { .. } => ErrorCode::DraftNoRosterSpace,
    };

    graphql_error(code, rejection.to_string())
}

/// Re-reads the round and contract a mutated selection needs for its GraphQL shape.
async fn selection_view(
    model: &rookie_draft_selection::Model,
    db: &DatabaseConnection,
) -> Result<RookieDraftSelection> {
    let draft_pick_model = find_draft_pick_by_id(model.draft_pick_id, db)
        .await
        .map_err(|err| internal("failed to load the selection's draft pick", &err))?;

    let contract_model = match model.contract_id {
        Some(contract_id) => Some(
            find_contract_by_id(contract_id, db)
                .await
                .map_err(|err| internal("failed to load the drafted contract", &err))?,
        ),
        None => None,
    };

    RookieDraftSelection::from_model(model, Some(draft_pick_model.round), contract_model.as_ref())
}

/// The `(id, is_league_player)` keys of players already drafted in this draft.
async fn drafted_player_keys(
    league_id: i64,
    end_of_season_year: i16,
    db: &DatabaseConnection,
) -> Result<Vec<(i64, bool)>> {
    let selections = get_selections_for_draft(league_id, end_of_season_year, db)
        .await
        .map_err(|err| internal("failed to load rookie draft selections", &err))?;
    let contract_ids = selections.iter().filter_map(|s| s.contract_id).collect();

    let contracts = find_contracts_by_ids(contract_ids, db)
        .await
        .map_err(|err| internal("failed to load rookie draft contracts", &err))?;

    Ok(contracts
        .iter()
        .filter_map(|contract_model| {
            contract_model.player_id.map_or_else(
                || {
                    contract_model
                        .league_player_id
                        .map(|league_player_id| (league_player_id, true))
                },
                |player_id| Some((player_id, false)),
            )
        })
        .collect())
}

const fn player_key(related_player: &RelatedPlayer) -> (i64, bool) {
    match related_player {
        RelatedPlayer::LeaguePlayer(model) => (model.id, true),
        RelatedPlayer::Player(model) => (model.id, false),
    }
}

/// Guarded so a stored round outside 1..=5 yields null instead of panicking mid-resolver.
fn salary_for_round(round: i16) -> Option<i16> {
    (1..=DRAFT_PICK_ROUNDS)
        .contains(&round)
        .then(|| rookie_draft_salary_for_round(round))
}

/// The caller's league plus the season to read, defaulting to the current one.
async fn league_and_season(
    ctx: &Context<'_>,
    end_of_season_year: Option<i16>,
) -> Result<(i64, i16)> {
    let (_, caller_team) = require_league_role(ctx, RoleRequirement::Member).await?;
    let season = match end_of_season_year {
        Some(year) => year,
        None => current_season(ctx, caller_team.league_id).await?,
    };
    Ok((caller_team.league_id, season))
}

fn internal(message: &str, error: &color_eyre::Report) -> async_graphql::Error {
    tracing::error!(error = ?error, message);
    code_error(ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(error: &GraphQlError) -> Option<async_graphql::Value> {
        error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .cloned()
    }

    #[test]
    fn every_pick_rejection_gets_its_own_code() {
        let cases = [
            (PickRejection::DraftNotStarted, "DRAFT_NOT_STARTED"),
            (
                PickRejection::NotOnTheClock {
                    selection_id: 1,
                    on_the_clock_order: 2,
                },
                "DRAFT_NOT_ON_THE_CLOCK",
            ),
            (
                PickRejection::SelectionAlreadyResolved { selection_id: 1 },
                "DRAFT_SELECTION_RESOLVED",
            ),
            (
                PickRejection::PlayerNotEligible,
                "DRAFT_PLAYER_NOT_ELIGIBLE",
            ),
            (PickRejection::ReDraftBanned, "DRAFT_RE_DRAFT_BANNED"),
            (
                PickRejection::NoRosterSpace {
                    roster_used: 32,
                    roster_limit: 32,
                },
                "DRAFT_NO_ROSTER_SPACE",
            ),
        ];

        for (rejection, expected_code) in cases {
            let error = pick_error(&Report::new(rejection));
            assert_eq!(code_of(&error), Some(expected_code.into()));
        }
    }

    #[test]
    fn other_pick_failures_stay_internal() {
        let error = pick_error(&color_eyre::eyre::eyre!("db exploded"));

        assert_eq!(code_of(&error), Some("INTERNAL".into()));
    }

    #[test]
    fn salary_is_null_outside_the_draft_rounds() {
        assert_eq!(salary_for_round(1), Some(4));
        assert_eq!(salary_for_round(5), Some(1));
        assert_eq!(salary_for_round(0), None);
        assert_eq!(salary_for_round(6), None);
    }
}
