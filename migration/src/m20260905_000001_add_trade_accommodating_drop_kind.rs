//! Records which move an owner declared with a trade, not only a drop (rules §13.1.5.3, §13.1.5.4).
//!
//! A move to the injured reserve frees an active roster slot and its cap space, so it makes room
//! for a trade the way a drop does. The table keeps its name because the row still means "the move
//! this owner submitted so the trade fits"; only the move itself is now a choice.
//!
//! Existing rows are all drops, which is what the default backfills.

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
            "ALTER TABLE trade_accommodating_drop
                ADD COLUMN kind VARCHAR NOT NULL DEFAULT 'Drop'",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(
            manager,
            "ALTER TABLE trade_accommodating_drop DROP COLUMN kind",
        )
        .await
    }
}
