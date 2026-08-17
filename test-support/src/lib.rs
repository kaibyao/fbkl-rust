//! DB-backed test harness: one scratch Postgres database per test, migrated from scratch.
//!
//! Every test gets its own database named after itself, so tests never see each other's rows and a
//! failed run leaves the database behind for inspection (the next run drops it). The base
//! connection string comes from `DATABASE_URL` (the repo `.env` is loaded automatically); when it
//! is unset the harness returns `None` and the test skips, so the suite still passes without a
//! database.
//!
//! # Simulated time vs `created_at`/`updated_at`
//!
//! Tests drive the scheduler by passing a simulated `now` into a tick, but `created_at` and
//! `updated_at` are stamped by the database clock — `updated_at` by the `set_updated_at` trigger on
//! every table. So any tick that compares a row's own timestamps against `now` sees a row written
//! "today" no matter which season the test is simulating, and the tick silently finds nothing to do.
//! A test that only asserts `summary.errors == 0` passes while covering none of the path.
//!
//! Rewind those columns before the tick that should act on them; `backdate_open_auctions` does it
//! for the veteran auction tier slide. The trigger only stamps the wall clock when the UPDATE did
//! not set `updated_at` itself, so writing it explicitly is what makes the rewind stick.
//!
//! # Growing the harness
//!
//! New helpers default to methods on [`TestLeague`]. When the impl block gets long, split it across
//! modules (`impl TestLeague` can live in more than one file) rather than inventing a type to hold
//! the overflow.
//!
//! A separate fixture struct earns its place only when it carries state `TestLeague` cannot — its
//! own ids or invariants — not merely because it covers a different domain. A struct that wraps
//! `TestLeague` and forwards every call is one implementation behind one interface, and costs a
//! layer of indirection for nothing. Trades are the case that will actually qualify: `team_id` here
//! is singular, so a two-team fixture needs a shape this struct cannot express.
//!
//! When that happens, borrow rather than rebuild — reach the new fixture through an accessor so it
//! shares the already-migrated database:
//!
//! ```ignore
//! pub struct TestTrade<'a> { league: &'a TestLeague, team_a: i64, team_b: i64 }
//!
//! impl TestLeague {
//!     pub fn trade(&self, team_a: i64, team_b: i64) -> TestTrade<'_> { .. }
//! }
//! ```

mod scratch_db;

use crate::scratch_db::scratch_db;
use fbkl_constants::date::league_wall_clock;
use fbkl_entity::{
    auction::{self, AuctionKind, AuctionStatus},
    auction_queries,
    auction_schedule_queries::{self, NewAuctionScheduleRow},
    contract::{self, ContractKind, ContractStatus},
    contract_queries,
    deadline::{self, DeadlineKind},
    draft_pick, league,
    player::{self, NbaRosterSource, PlayerStatus},
    position, real_team,
    sea_orm::{
        ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
        prelude::{Date, DateTimeWithTimeZone, Expr},
    },
    team,
    team_user::{self, LeagueRole},
    trade::{self, TradeStatus},
    trade_action::{self, TradeActionType},
    trade_asset::{self, FromTeamId, ToTeamId},
    user,
};

/// A migrated scratch database holding one league, one team, and one real NBA team to hang
/// players off.
pub struct TestLeague {
    pub db: DatabaseConnection,
    pub league_id: i64,
    pub team_id: i64,
    pub real_team_id: i64,
    pub position_id: i32,
    pub end_of_season_year: i16,
}

impl TestLeague {
    /// `None` when no `DATABASE_URL` is configured, which the caller should treat as "skip".
    pub async fn create(test_name: &str, end_of_season_year: i16) -> Option<Self> {
        let db = scratch_db(test_name).await?;

        let league_id = league::Entity::insert(league::ActiveModel {
            name: ActiveValue::Set(format!("Test league ({test_name})")),
            ..Default::default()
        })
        .exec(&db)
        .await
        .expect("insert league")
        .last_insert_id;

        let team_id = team::Entity::insert(team::ActiveModel {
            name: ActiveValue::Set("Test team".to_owned()),
            league_id: ActiveValue::Set(league_id),
            ..Default::default()
        })
        .exec(&db)
        .await
        .expect("insert team")
        .last_insert_id;

        let real_team_id = real_team::Entity::insert(real_team::ActiveModel {
            city: ActiveValue::Set("Testville".to_owned()),
            name: ActiveValue::Set("Testers".to_owned()),
            code: ActiveValue::Set("TST".to_owned()),
            espn_id: ActiveValue::Set(1),
            nba_id: ActiveValue::Set(1),
            logo_url: ActiveValue::Set(String::new()),
            ..Default::default()
        })
        .exec(&db)
        .await
        .expect("insert real team")
        .last_insert_id;

        // Positions are seeded by migration; any of them satisfies the player FK.
        let position_id = position::Entity::find()
            .one(&db)
            .await
            .expect("find position")
            .expect("migrations seed positions")
            .id;

        Some(Self {
            db,
            league_id,
            team_id,
            real_team_id,
            position_id,
            end_of_season_year,
        })
    }

    pub async fn add_deadline(&self, kind: DeadlineKind, date_time: DateTimeWithTimeZone) {
        deadline::Entity::insert(deadline::ActiveModel {
            date_time: ActiveValue::Set(date_time),
            kind: ActiveValue::Set(kind),
            name: ActiveValue::Set(format!("{kind:?}")),
            end_of_season_year: ActiveValue::Set(self.end_of_season_year),
            league_id: ActiveValue::Set(self.league_id),
            ..Default::default()
        })
        .exec(&self.db)
        .await
        .expect("insert deadline");
    }

    /// Minimum-bid tiers in the given order, top tier first (rules §6.3.6).
    pub async fn add_min_bid_tiers(&self, min_bid_amounts: &[i16]) {
        auction_schedule_queries::set_min_bid_tiers(
            self.league_id,
            self.end_of_season_year,
            min_bid_amounts,
            &self.db,
        )
        .await
        .expect("set min bid tiers");
    }

    /// The season's ranked nomination list, best player first (rules §6.3.2).
    pub async fn add_ranked_players(&self, ranked_player_ids: &[i64]) {
        auction_schedule_queries::set_veteran_auction_ranking(
            self.league_id,
            self.end_of_season_year,
            ranked_player_ids,
            &self.db,
        )
        .await
        .expect("set veteran auction ranking");
    }

    /// A member of the test team in the given league role, i.e. who acts on the team's behalf.
    ///
    /// One member per role: the user's email is derived from the role so a second call with the
    /// same one fails loudly instead of silently seeding a duplicate owner.
    pub async fn add_team_user(&self, league_role: LeagueRole) -> team_user::Model {
        self.add_team_user_for_team(self.team_id, league_role).await
    }

    /// [`Self::add_team_user`] for one of the extra teams [`Self::add_team`] made.
    pub async fn add_team_user_for_team(
        &self,
        team_id: i64,
        league_role: LeagueRole,
    ) -> team_user::Model {
        let user_id = user::Entity::insert(user::ActiveModel {
            email: ActiveValue::Set(format!("{league_role:?}-{team_id}@example.com")),
            hashed_password: ActiveValue::Set("not-a-real-hash".to_owned()),
            ..Default::default()
        })
        .exec(&self.db)
        .await
        .expect("insert user")
        .last_insert_id;

        team_user::ActiveModel {
            league_role: ActiveValue::Set(league_role),
            nickname: ActiveValue::Set(format!("Test {league_role:?}")),
            first_end_of_season_year: ActiveValue::Set(self.end_of_season_year),
            team_id: ActiveValue::Set(team_id),
            user_id: ActiveValue::Set(user_id),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .expect("insert team user")
    }

    /// Rewinds every open auction's start and update timestamps, i.e. simulates a day passing with
    /// nobody touching them.
    ///
    /// `auction.updated_at` is stamped by the database clock, so a test driving ticks from
    /// simulated timestamps can never otherwise satisfy the tier slide's untouched-for-a-day bound.
    /// The write sets `updated_at` itself, which is what keeps the `set_updated_at` trigger from
    /// stamping the row back to the wall clock.
    pub async fn backdate_open_auctions(&self, timestamp: DateTimeWithTimeZone) {
        auction::Entity::update_many()
            .col_expr(auction::Column::StartTimestamp, Expr::value(timestamp))
            .col_expr(auction::Column::UpdatedAt, Expr::value(timestamp))
            .filter(auction::Column::Status.eq(AuctionStatus::Open))
            .exec(&self.db)
            .await
            .expect("backdate open auctions");
    }

    /// The veteran auction opened for `player_id`, whatever state it has since reached.
    pub async fn find_veteran_auction(&self, player_id: i64) -> Option<auction::Model> {
        auction_queries::find_auction_for_player_in_season(
            self.league_id,
            self.end_of_season_year,
            player_id,
            AuctionKind::PreseasonVeteranAuction,
            &self.db,
        )
        .await
        .expect("find veteran auction")
    }

    /// One veteran-auction release, as pool assembly would have written it (rules §6.3.3).
    pub async fn add_schedule_row(
        &self,
        player_id: i64,
        scheduled_release_date: Date,
        min_bid_tier: i16,
    ) {
        auction_schedule_queries::insert_auction_schedule_rows(
            self.league_id,
            self.end_of_season_year,
            vec![NewAuctionScheduleRow {
                player_id,
                scheduled_release_date,
                nomination_rank: None,
                min_bid_tier,
                is_rfa_week: false,
            }],
            &self.db,
        )
        .await
        .expect("insert auction schedule row");
    }

    /// A player who has played in an earlier NBA season, i.e. veteran-auction-eligible (§3.1.2).
    pub async fn add_veteran_player(&self, name: &str) -> i64 {
        player::Entity::insert(player::ActiveModel {
            is_rdi_eligible: ActiveValue::Set(false),
            name: ActiveValue::Set(name.to_owned()),
            position_id: ActiveValue::Set(self.position_id),
            status: ActiveValue::Set(PlayerStatus::Active),
            has_played_nba_game: ActiveValue::Set(true),
            nba_first_season_end_of_season_year: ActiveValue::Set(Some(
                self.end_of_season_year - 3,
            )),
            nba_roster_source: ActiveValue::Set(NbaRosterSource::Nba),
            current_real_team_id: ActiveValue::Set(self.real_team_id),
            ..Default::default()
        })
        .exec(&self.db)
        .await
        .expect("insert player")
        .last_insert_id
    }

    /// An unowned (`team_id` NULL) active contract, i.e. how an RFA/UFA sits going into the auction.
    pub async fn add_unowned_contract(
        &self,
        player_id: i64,
        kind: ContractKind,
        salary: i16,
    ) -> contract::Model {
        contract_queries::create_new_contract(
            contract::ActiveModel {
                year_number: ActiveValue::Set(4),
                kind: ActiveValue::Set(kind),
                is_ir: ActiveValue::Set(false),
                salary: ActiveValue::Set(salary),
                end_of_season_year: ActiveValue::Set(self.end_of_season_year),
                status: ActiveValue::Set(ContractStatus::Active),
                league_id: ActiveValue::Set(self.league_id),
                player_id: ActiveValue::Set(Some(player_id)),
                ..Default::default()
            },
            &self.db,
        )
        .await
        .expect("insert unowned contract")
    }

    /// The same contract owned by `owner_team_id`, i.e. how a designated RFA/UFA sits at the
    /// keeper deadline before the auction pool is assembled.
    pub async fn add_owned_contract(
        &self,
        player_id: i64,
        kind: ContractKind,
        salary: i16,
        owner_team_id: i64,
    ) -> contract::Model {
        let mut contract_to_own: contract::ActiveModel = self
            .add_unowned_contract(player_id, kind, salary)
            .await
            .into();
        contract_to_own.team_id = ActiveValue::Set(Some(owner_team_id));
        contract_to_own
            .update(&self.db)
            .await
            .expect("own contract")
    }

    /// A second team in the same league, for cases that need an asset to change hands.
    pub async fn add_team(&self, name: &str) -> i64 {
        team::Entity::insert(team::ActiveModel {
            name: ActiveValue::Set(name.to_owned()),
            league_id: ActiveValue::Set(self.league_id),
            ..Default::default()
        })
        .exec(&self.db)
        .await
        .expect("insert team")
        .last_insert_id
    }

    /// A Rookie-Draft pick for this season, held by `owner_team_id` since it was created.
    pub async fn add_draft_pick(&self, round: i16, owner_team_id: i64) -> draft_pick::Model {
        draft_pick::ActiveModel {
            round: ActiveValue::Set(round),
            end_of_season_year: ActiveValue::Set(self.end_of_season_year),
            league_id: ActiveValue::Set(self.league_id),
            current_owner_team_id: ActiveValue::Set(owner_team_id),
            original_owner_team_id: ActiveValue::Set(owner_team_id),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .expect("insert draft pick")
    }

    /// Records a pick trade that has already gone through, announced at `accepted_at`.
    ///
    /// This writes the rows a processed trade leaves behind — a completed `trade`, its pick
    /// `trade_asset`, the accepting `trade_action`, and the pick's new owner — without running the
    /// trade engine, which would need rosters, salaries and a deadline the test does not care about.
    /// `accepted_at` is written explicitly because the announcement time is what pick eligibility
    /// reads, and the database clock would otherwise stamp it "now".
    pub async fn add_completed_pick_trade(
        &self,
        draft_pick_model: &draft_pick::Model,
        to_team_id: i64,
        accepting_team_user_id: i64,
        accepted_at: DateTimeWithTimeZone,
    ) {
        let trade_model = trade::ActiveModel {
            end_of_season_year: ActiveValue::Set(self.end_of_season_year),
            status: ActiveValue::Set(TradeStatus::Completed),
            league_id: ActiveValue::Set(self.league_id),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .expect("insert trade");

        trade_asset::Model::from_draft_pick(
            Some(trade_model.id),
            draft_pick_model.id,
            FromTeamId(draft_pick_model.current_owner_team_id),
            ToTeamId(to_team_id),
        )
        .insert(&self.db)
        .await
        .expect("insert pick trade asset");

        trade_action::ActiveModel {
            action_type: ActiveValue::Set(TradeActionType::Accept),
            team_user_id: ActiveValue::Set(accepting_team_user_id),
            trade_id: ActiveValue::Set(trade_model.id),
            created_at: ActiveValue::Set(accepted_at),
            ..Default::default()
        }
        .insert(&self.db)
        .await
        .expect("insert accepting trade action");

        let mut traded_pick: draft_pick::ActiveModel = draft_pick_model.clone().into();
        traded_pick.current_owner_team_id = ActiveValue::Set(to_team_id);
        traded_pick.update(&self.db).await.expect("transfer pick");
    }
}

/// Parses a CT wall clock written as `YYYY-MM-DDTHH:MM:SS`, the timezone every league deadline
/// uses. DST included, so a September timestamp lands on CDT and a January one on CST.
pub fn central(timestamp: &str) -> DateTimeWithTimeZone {
    league_wall_clock(timestamp.parse().expect("parse timestamp")).expect("central wall clock")
}
