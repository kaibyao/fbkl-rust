//! Adds the eligibility model of spec 10 to `player` + `league_player`.
//!
//! `has_been_on_nba_roster` is the rules §3.1.2 pivot that splits the veteran auction pool from
//! the rookie draft pool. The classification itself is NOT stored — it is derived in
//! `logic::eligibility` — so only the commissioner override (plus its audit trail) is persisted.

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
                        ADD COLUMN has_been_on_nba_roster BOOLEAN NOT NULL DEFAULT FALSE,
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
            run_sql(
                manager,
                &format!(
                    "CREATE INDEX {table}_has_been_on_nba_roster ON {table} (has_been_on_nba_roster)"
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
                        DROP COLUMN has_been_on_nba_roster,
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
