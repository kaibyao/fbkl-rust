//! Adds the owner-chosen ordering of one week's roster moves to `team_update` (rules §13.1.1).
//!
//! An owner may re-order the transactions they made in a single week however they like, so the
//! order is a property of the team update rather than of its insertion time. It is presentational
//! and for the audit log only: legality is computed over the final projected roster, never over an
//! intermediate ordering, so no validator reads this column.
//!
//! The column is nullable with no backfill, because a move made outside the weekly tray (a trade,
//! an auction win, anything the processor writes) has no owner-chosen place. Those rows fall back
//! to insertion order when the week is rendered.
//!
//! Superseded: `m20260902_000002` renames the column to `transaction_number`, and validators do
//! read it now (rules §13.1.6).

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
            "ALTER TABLE team_update ADD COLUMN sequence SMALLINT",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(manager, "ALTER TABLE team_update DROP COLUMN sequence").await
    }
}
