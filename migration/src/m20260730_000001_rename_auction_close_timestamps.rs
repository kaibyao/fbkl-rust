//! Retimes `auction`'s two end timestamps (rules §6.4.4 / §8.3.1-.2, spec 01 "Timing rules").
//!
//! `soft_end_timestamp` becomes `close_at_timestamp`: the one instant an auction stops taking bids,
//! `min(last_bid + quiet_window, all_bid_deadline)` clamped to the hard deadline. Both the close
//! tick and the bid path compare against it, so the close tick stays one indexed scan.
//!
//! `fixed_end_timestamp` becomes `all_bid_deadline_timestamp` and drops NOT NULL. Only in-season FA
//! auctions have an all-bid deadline (Sunday 8pm CT, rolled +30min per §8.3.2); the old NOT NULL
//! column defaulted to start+48h for every auction, which cut off live veteran bidding. Existing
//! non-FA rows are nulled out so the invariant holds in the data too — they are all settled
//! historical auctions whose old value was that meaningless start+48h.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{DatabaseBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

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
        run_sql(
            manager,
            "ALTER TABLE auction
                RENAME COLUMN soft_end_timestamp TO close_at_timestamp",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction
                RENAME COLUMN fixed_end_timestamp TO all_bid_deadline_timestamp",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction
                ALTER COLUMN all_bid_deadline_timestamp DROP NOT NULL",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE auction SET all_bid_deadline_timestamp = NULL WHERE kind <> 'FreeAgent'",
        )
        .await?;
        run_sql(
            manager,
            "CREATE INDEX auction_close_at ON auction (close_at_timestamp)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(manager, "DROP INDEX auction_close_at").await?;
        // NOT NULL needs every row filled; the close time is the closest thing to the old value.
        run_sql(
            manager,
            "UPDATE auction
                SET all_bid_deadline_timestamp = close_at_timestamp
                WHERE all_bid_deadline_timestamp IS NULL",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction
                ALTER COLUMN all_bid_deadline_timestamp SET NOT NULL",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction
                RENAME COLUMN all_bid_deadline_timestamp TO fixed_end_timestamp",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction
                RENAME COLUMN close_at_timestamp TO soft_end_timestamp",
        )
        .await
    }
}
