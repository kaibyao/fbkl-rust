//! Adds the spec 01 auction lifecycle to `auction`.
//!
//! `status` replaces the old implicit "open if now < `soft_end_timestamp`" inference, so the
//! per-minute close tick and the bid path can gate on an explicit state. Existing rows are
//! backfilled from `transaction_id`: a settled auction is `Completed`, an unsettled historical one
//! never got a winning bid, so it is `Expired`.
//!
//! `original_owner_team_id` is set only for RFA/UFA auctions (rules §6.2.2.3 / §15.3.1) — the
//! engine rejects that team's bids and routes RFA closes to the spec 03 raise/match flow.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{DatabaseBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

async fn run_sql(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map(|_| ())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(
            manager,
            "ALTER TABLE auction
                ADD COLUMN status VARCHAR NOT NULL DEFAULT 'Open',
                ADD COLUMN original_owner_team_id BIGINT",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE auction SET status = CASE WHEN transaction_id IS NULL THEN 'Expired' ELSE 'Completed' END",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction ADD CONSTRAINT auction_fk_original_owner_team
                FOREIGN KEY (original_owner_team_id) REFERENCES team(id)
                ON UPDATE CASCADE ON DELETE SET NULL",
        )
        .await?;
        run_sql(manager, "CREATE INDEX auction_status ON auction (status)").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(
            manager,
            "ALTER TABLE auction DROP COLUMN status, DROP COLUMN original_owner_team_id",
        )
        .await
    }
}
