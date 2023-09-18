use fbkl_entity::sea_orm::TransactionTrait;
use sea_orm_migration::prelude::*;

use crate::{m20221023_002184_create_draft_pick::DraftPick, set_auto_updated_at_on_table};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;

        setup_draft_pick_option(manager).await?;
        setup_draft_pick_draft_pick_option(manager).await?;

        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(DraftPickDraftPickOption::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(DraftPickOption::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn setup_draft_pick_option(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(DraftPickOption::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(DraftPickOption::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(DraftPickOption::Clause).string().not_null())
                .col(
                    ColumnDef::new(DraftPickOption::Status)
                        .string()
                        .not_null()
                        .default("Proposed"),
                )
                .col(
                    ColumnDef::new(DraftPickOption::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(DraftPickOption::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, DraftPickOption::Table.to_string()).await
}

async fn setup_draft_pick_draft_pick_option(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(DraftPickDraftPickOption::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(DraftPickDraftPickOption::DraftPickId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DraftPickDraftPickOption::DraftPickOptionId)
                        .big_integer()
                        .not_null(),
                )
                .primary_key(
                    IndexCreateStatement::new()
                        .name("draft_pick_to_option_pk")
                        .table(DraftPickDraftPickOption::Table)
                        .col(DraftPickDraftPickOption::DraftPickId)
                        .col(DraftPickDraftPickOption::DraftPickOptionId),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("draft_pick_to_option_m2m_draft_pick_option_id")
                .table(DraftPickDraftPickOption::Table)
                .col(DraftPickDraftPickOption::DraftPickOptionId)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("draft_pick_to_option_m2m_fk_draft_pick")
                .from(
                    DraftPickDraftPickOption::Table,
                    DraftPickDraftPickOption::DraftPickId,
                )
                .to(DraftPick::Table, DraftPick::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("draft_pick_to_option_m2m_fk_draft_pick_option")
                .from(
                    DraftPickDraftPickOption::Table,
                    DraftPickDraftPickOption::DraftPickOptionId,
                )
                .to(DraftPickOption::Table, DraftPickOption::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum DraftPickOption {
    Table,
    Id,
    Clause,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum DraftPickDraftPickOption {
    Table,
    DraftPickId,
    DraftPickOptionId,
}
