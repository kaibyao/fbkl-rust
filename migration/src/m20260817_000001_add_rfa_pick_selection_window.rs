//! Gives the RFA winner his own window to name the compensation pick (rules §15.2.2).
//!
//! Rules §15.2.2 lets the winner choose which eligible pick he forfeits, so the choice needs a
//! moment of its own between the raise window closing and the original owner's match window
//! opening. `pick_selection_deadline_at` is that window's clock; it is NULL until the raise window
//! settles, and it stays NULL for a resolution nobody bid on.

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
            "ALTER TABLE rfa_resolution ADD COLUMN IF NOT EXISTS pick_selection_deadline_at TIMESTAMP WITH TIME ZONE",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The status has no column of its own to roll back, so the rows parked in it are dropped.
        run_sql(
            manager,
            "DELETE FROM rfa_resolution WHERE status = 'AwaitingPickSelection'",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE rfa_resolution DROP COLUMN IF EXISTS pick_selection_deadline_at",
        )
        .await
    }
}
