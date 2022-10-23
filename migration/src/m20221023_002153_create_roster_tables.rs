use fbkl_entity::sea_orm::TransactionTrait;
use sea_orm_migration::prelude::*;

use crate::{m20220924_004529_create_league_tables::Team, set_auto_updated_at_on_table};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;

        setup_roster(manager).await?;
        setup_roster_update(manager).await?;

        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RosterUpdate::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Roster::Table).if_exists().to_owned())
            .await
    }
}

async fn setup_roster(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Roster::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Roster::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Roster::SalaryCap)
                        .small_integer()
                        .not_null()
                        .default(200),
                )
                .col(
                    ColumnDef::new(Roster::SeasonEndYear)
                        .small_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Roster::TeamId).big_integer().not_null())
                .col(
                    ColumnDef::new(Roster::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Roster::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Roster::Table.to_string()).await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("roster_year_team")
                .table(Roster::Table)
                .col(Roster::SeasonEndYear)
                .col(Roster::TeamId)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("roster_fk_team")
                .from(Roster::Table, Roster::TeamId)
                .to(Team::Table, Team::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await
}

async fn setup_roster_update(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(RosterUpdate::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(RosterUpdate::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(RosterUpdate::RosterId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RosterUpdate::Status)
                        .small_integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(RosterUpdate::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(RosterUpdate::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, RosterUpdate::Table.to_string()).await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("roster_update_roster_id")
                .table(RosterUpdate::Table)
                .col(RosterUpdate::RosterId)
                .col(RosterUpdate::Status)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("roster_update_fk_roster")
                .from(RosterUpdate::Table, RosterUpdate::RosterId)
                .to(Roster::Table, Roster::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Roster {
    Table,
    Id,
    SalaryCap,
    SeasonEndYear,
    TeamId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum RosterUpdate {
    Table,
    Id,
    RosterId,
    Status,
    CreatedAt,
    UpdatedAt,
}
