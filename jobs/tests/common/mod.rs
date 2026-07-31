//! DB-backed test harness: one scratch Postgres database per test, migrated from scratch.
//!
//! Every test gets its own database named after itself, so tests never see each other's rows and a
//! failed run leaves the database behind for inspection (the next run drops it). The base
//! connection string comes from `DATABASE_URL` (the repo `.env` is loaded automatically); when it
//! is unset the harness returns `None` and the test skips, so the suite still passes without a
//! database.

#![allow(dead_code)] // Shared across test binaries; not every binary uses every helper.

use fbkl_entity::{
    auction::{self, AuctionKind},
    auction_queries,
    auction_schedule_queries::{self, NewAuctionScheduleRow},
    contract::{self, ContractKind, ContractStatus},
    contract_queries,
    deadline::{self, DeadlineKind},
    league, min_bid_tier_config,
    player::{self, NbaRosterSource, PlayerStatus},
    position, real_team,
    sea_orm::{
        ActiveValue, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
        prelude::{Date, DateTimeWithTimeZone},
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
        for (tier_index, min_bid_amount) in min_bid_amounts.iter().enumerate() {
            min_bid_tier_config::Entity::insert(min_bid_tier_config::ActiveModel {
                league_id: ActiveValue::Set(self.league_id),
                end_of_season_year: ActiveValue::Set(self.end_of_season_year),
                tier_index: ActiveValue::Set(i16::try_from(tier_index).expect("tier index fits")),
                min_bid_amount: ActiveValue::Set(*min_bid_amount),
                ..Default::default()
            })
            .exec(&self.db)
            .await
            .expect("insert min bid tier");
        }
    }

    /// An owner of the test team, i.e. the `team_user_id` an auction bid is placed under.
    pub async fn add_team_user(&self) -> i64 {
        let user_id = user::Entity::insert(user::ActiveModel {
            email: ActiveValue::Set(format!("owner{}@example.com", self.team_id)),
            hashed_password: ActiveValue::Set("not-a-real-hash".to_owned()),
            ..Default::default()
        })
        .exec(&self.db)
        .await
        .expect("insert user")
        .last_insert_id;

        team_user::Entity::insert(team_user::ActiveModel {
            league_role: ActiveValue::Set(LeagueRole::TeamOwner),
            nickname: ActiveValue::Set("Test owner".to_owned()),
            first_end_of_season_year: ActiveValue::Set(self.end_of_season_year),
            team_id: ActiveValue::Set(self.team_id),
            user_id: ActiveValue::Set(user_id),
            ..Default::default()
        })
        .exec(&self.db)
        .await
        .expect("insert team user")
        .last_insert_id
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
