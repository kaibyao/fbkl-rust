use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::League,
    m20221117_235325_create_transaction::{Deadline, Transaction},
    set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(JobRun::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(JobRun::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(JobRun::LeagueId).big_integer().not_null())
                    .col(
                        ColumnDef::new(JobRun::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(JobRun::DeadlineId).big_integer())
                    .col(ColumnDef::new(JobRun::EventKind).string().not_null())
                    .col(ColumnDef::new(JobRun::DispatchTarget).string().not_null())
                    .col(
                        ColumnDef::new(JobRun::Status)
                            .string()
                            .not_null()
                            .default("Pending"),
                    )
                    .col(
                        ColumnDef::new(JobRun::Attempts)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(JobRun::IdempotencyKey).string().not_null())
                    .col(ColumnDef::new(JobRun::TransactionId).big_integer())
                    .col(ColumnDef::new(JobRun::Error).text())
                    .col(
                        ColumnDef::new(JobRun::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(JobRun::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, JobRun::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("job_run_fk_league")
                    .from(JobRun::Table, JobRun::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("job_run_fk_deadline")
                    .from(JobRun::Table, JobRun::DeadlineId)
                    .to(Deadline::Table, Deadline::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("job_run_fk_transaction")
                    .from(JobRun::Table, JobRun::TransactionId)
                    .to(Transaction::Table, Transaction::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // The unique index is the double-fire guard: a concurrent or re-fired tick
        // attempting to insert the same idempotency_key conflicts instead of double-processing.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("job_run_idempotency_key")
                    .table(JobRun::Table)
                    .col(JobRun::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("job_run_deadline_status")
                    .table(JobRun::Table)
                    .col(JobRun::DeadlineId)
                    .col(JobRun::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("job_run_league_season")
                    .table(JobRun::Table)
                    .col(JobRun::LeagueId)
                    .col(JobRun::EndOfSeasonYear)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(JobRun::Table).if_exists().to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum JobRun {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    DeadlineId,
    EventKind,
    DispatchTarget,
    Status,
    Attempts,
    IdempotencyKey,
    TransactionId,
    Error,
    CreatedAt,
    UpdatedAt,
}
