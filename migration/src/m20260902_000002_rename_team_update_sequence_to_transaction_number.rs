//! Renames `team_update.sequence` to `team_update.transaction_number` (rules §13.1.4-§13.1.6).
//!
//! The column was added as the owner's chosen place for one move in its week. Under the
//! transaction model it holds which transaction of the week a move belongs to: rows sharing a
//! value are one transaction, transactions apply in ascending order, and a roster is judged after
//! each of them. "Sequence" would have to mean both a position and a grouping key, so the column
//! takes the name of the thing it now identifies.
//!
//! No index, constraint or default embeds the old name, so one `RENAME COLUMN` covers it.

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
            "ALTER TABLE team_update RENAME COLUMN sequence TO transaction_number",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(
            manager,
            "ALTER TABLE team_update RENAME COLUMN transaction_number TO sequence",
        )
        .await
    }
}
