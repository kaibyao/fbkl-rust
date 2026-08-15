//! Lets an RFA resolution exist before its auction closes (rules §15.4.2).
//!
//! The keeper deadline is when the original owner is decided, so the resolution row is written
//! there — months before any bid. The raise window only starts at auction close, so
//! `raise_deadline_at` has nothing to hold until then and becomes nullable. The new
//! `AwaitingAuction` status marks that pre-auction stretch; the scheduler's expiry query ignores
//! it, and a NULL deadline never compares as expired either.

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
            "ALTER TABLE rfa_resolution ALTER COLUMN raise_deadline_at DROP NOT NULL",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(
            manager,
            "DELETE FROM rfa_resolution WHERE raise_deadline_at IS NULL",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE rfa_resolution ALTER COLUMN raise_deadline_at SET NOT NULL",
        )
        .await
    }
}
