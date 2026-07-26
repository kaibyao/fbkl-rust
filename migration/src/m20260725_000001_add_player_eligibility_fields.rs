//! Adds the eligibility model of spec 10 to `player` + `league_player`.
//!
//! Two distinct NBA facts, because the rules use them for different questions: whether the player
//! has appeared in a game is the §3.1.2 pivot splitting the veteran auction pool from the rookie
//! draft pool, while the broader "has been on an NBA roster" (§3.1.3) only gates RDI eligibility
//! (§11.3). A rostered-but-never-played player is rookie-draft-eligible yet RDI-ineligible.
//!
//! Both facts are stored **as of a season**, not as career booleans, because eligibility is a
//! point-in-time question: the same player is rookie-draft-eligible in the season before his debut
//! and a veteran after it, and historical replay asks about past seasons.
//! `nba_first_season_end_of_season_year` is the season the player first appeared in NBA data, so
//! `<= the season in question` answers §3.1.3, and `has_played_nba_game` narrows that to §3.1.2.
//!
//! The classification itself is NOT stored — it is derived in `logic::eligibility` — so only the
//! commissioner override (plus its audit trail) is persisted.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{DatabaseBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLES: [&str; 2] = ["player", "league_player"];

async fn run_sql(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map(|_| ())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in TABLES {
            run_sql(
                manager,
                &format!(
                    "ALTER TABLE {table}
                        ADD COLUMN has_played_nba_game BOOLEAN NOT NULL DEFAULT FALSE,
                        ADD COLUMN nba_first_season_end_of_season_year SMALLINT,
                        ADD COLUMN nba_roster_source VARCHAR NOT NULL DEFAULT 'Unknown',
                        ADD COLUMN nba_roster_asof TIMESTAMPTZ,
                        ADD COLUMN eligibility_override VARCHAR,
                        ADD COLUMN eligibility_override_reason TEXT,
                        ADD COLUMN eligibility_override_by_team_user_id BIGINT,
                        ADD COLUMN eligibility_override_at TIMESTAMPTZ"
                ),
            )
            .await?;
            run_sql(
                manager,
                &format!(
                    "ALTER TABLE {table} ADD CONSTRAINT {table}_fk_eligibility_override_by_team_user
                        FOREIGN KEY (eligibility_override_by_team_user_id) REFERENCES team_user(id)
                        ON UPDATE CASCADE ON DELETE SET NULL"
                ),
            )
            .await?;
            // Pool assembly range-scans this to drop players with no NBA data as of the season.
            run_sql(
                manager,
                &format!(
                    "CREATE INDEX {table}_nba_first_season ON {table} (nba_first_season_end_of_season_year)"
                ),
            )
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in TABLES {
            run_sql(
                manager,
                &format!(
                    "ALTER TABLE {table}
                        DROP COLUMN has_played_nba_game,
                        DROP COLUMN nba_first_season_end_of_season_year,
                        DROP COLUMN nba_roster_source,
                        DROP COLUMN nba_roster_asof,
                        DROP COLUMN eligibility_override,
                        DROP COLUMN eligibility_override_reason,
                        DROP COLUMN eligibility_override_by_team_user_id,
                        DROP COLUMN eligibility_override_at"
                ),
            )
            .await?;
        }

        Ok(())
    }
}
