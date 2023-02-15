use sea_orm_migration::prelude::*;

use crate::{m20220924_004529_create_league_tables::League, set_auto_updated_at_on_table};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        setup_deadline(manager).await?;
        setup_transaction(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Transaction::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn setup_deadline(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Deadline::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Deadline::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Deadline::DateTime)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Deadline::DeadlineType)
                        .small_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Deadline::Name).string().not_null())
                .col(
                    ColumnDef::new(Deadline::SeasonEndYear)
                        .small_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Deadline::LeagueId).big_integer().not_null())
                .col(
                    ColumnDef::new(Deadline::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Deadline::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Deadline::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("deadline_fk_league")
                .from(Deadline::Table, Deadline::LeagueId)
                .to(League::Table, League::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("deadline_league_year")
                .table(Deadline::Table)
                .col(Deadline::LeagueId)
                .col(Deadline::SeasonEndYear)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("deadline_type")
                .table(Deadline::Table)
                .col(Deadline::DeadlineType)
                .to_owned(),
        )
        .await
}

async fn setup_transaction(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Transaction::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Transaction::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Transaction::SeasonEndYear)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Transaction::TransactionType)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Transaction::LeagueId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Transaction::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Transaction::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Transaction::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("transaction_fk_league")
                .from(Transaction::Table, Transaction::LeagueId)
                .to(League::Table, League::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("transaction_league_year")
                .table(Transaction::Table)
                .col(Transaction::LeagueId)
                .col(Transaction::SeasonEndYear)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("transaction_type")
                .table(Transaction::Table)
                .col(Transaction::TransactionType)
                .to_owned(),
        )
        .await
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum Deadline {
    Table,
    Id,
    DateTime,
    DeadlineType,
    Name,
    SeasonEndYear,
    LeagueId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum Transaction {
    Table,
    Id,
    SeasonEndYear,
    TransactionType,
    LeagueId,
    CreatedAt,
    UpdatedAt,
}
