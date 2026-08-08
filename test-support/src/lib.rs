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

use fbkl_entity::{
    auction::{self, AuctionKind, AuctionStatus},
    auction_queries,
    auction_schedule_queries::{self, NewAuctionScheduleRow},
    contract::{self, ContractKind, ContractStatus},
    contract_queries,
    deadline::{self, DeadlineKind},
    league,
    player::{self, NbaRosterSource, PlayerStatus},
    position, real_team,
    sea_orm::{
        ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, Database, DatabaseConnection,
        EntityTrait, QueryFilter,
        prelude::{Date, DateTimeWithTimeZone, Expr},
    },
    team,
    team_user::{self, LeagueRole},
    user,
};
use fbkl_migration::{Migrator, MigratorTrait};

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
        let user_id = user::Entity::insert(user::ActiveModel {
            email: ActiveValue::Set(format!("{league_role:?}-{}@example.com", self.team_id)),
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
            team_id: ActiveValue::Set(self.team_id),
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
}

/// Parses a CT timestamp written as `YYYY-MM-DDTHH:MM:SS`, the timezone every league deadline uses.
pub fn central(timestamp: &str) -> DateTimeWithTimeZone {
    format!("{timestamp}-06:00")
        .parse()
        .expect("parse timestamp")
}

/// Drops and recreates `fbkl_test_<test_name>` next to `DATABASE_URL`, then migrates it.
async fn scratch_db(test_name: &str) -> Option<DatabaseConnection> {
    dotenvy::dotenv().ok();
    let Ok(base_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping {test_name}: DATABASE_URL not set");
        return None;
    };

    let (host_url, _) = base_url
        .trim_end_matches('/')
        .rsplit_once('/')
        .expect("DATABASE_URL must end in a database name");
    let scratch_name = format!("fbkl_test_{test_name}");

    let admin_db = Database::connect(format!("{host_url}/postgres"))
        .await
        .expect("connect to the postgres maintenance database");
    admin_db
        .execute_unprepared(&format!(
            "DROP DATABASE IF EXISTS {scratch_name} WITH (FORCE)"
        ))
        .await
        .expect("drop scratch database");
    admin_db
        .execute_unprepared(&format!("CREATE DATABASE {scratch_name}"))
        .await
        .expect("create scratch database");

    let db = Database::connect(format!("{host_url}/{scratch_name}"))
        .await
        .expect("connect to scratch database");
    Migrator::up(&db, None)
        .await
        .expect("migrate scratch database");
    Some(db)
}
