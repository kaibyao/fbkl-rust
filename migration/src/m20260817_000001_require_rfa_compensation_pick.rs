//! Makes the named compensation pick required (rules §15.3.3).
//!
//! A bid on a restricted free agent names the pick it would cost as it is placed, so the row is
//! never written without one. The column was nullable while the pick was named in a window of its
//! own, after the bid.

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
        // Rows without a pick were debts nobody had chosen for yet, which the new flow cannot make.
        run_sql(
            manager,
            "DELETE FROM rfa_compensation_pick WHERE forfeited_draft_pick_id IS NULL",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE rfa_compensation_pick ALTER COLUMN forfeited_draft_pick_id SET NOT NULL",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(
            manager,
            "ALTER TABLE rfa_compensation_pick ALTER COLUMN forfeited_draft_pick_id DROP NOT NULL",
        )
        .await
    }
}
