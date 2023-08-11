use sea_orm_migration::prelude::*;

use crate::{m20221023_002183_create_asset_tables::DraftPickOption, set_auto_updated_at_on_table};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DraftPickOptionAmendment::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DraftPickOptionAmendment::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DraftPickOptionAmendment::DraftPickOptionId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(DraftPickOptionAmendment::AmendedClause).string())
                    .col(
                        ColumnDef::new(DraftPickOptionAmendment::AmendmentType)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DraftPickOptionAmendment::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(DraftPickOptionAmendment::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, DraftPickOptionAmendment::Table.to_string()).await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("draft_pick_option_amendment_option_id")
                    .table(DraftPickOptionAmendment::Table)
                    .col(DraftPickOptionAmendment::DraftPickOptionId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("draft_pick_option_amendment_type")
                    .table(DraftPickOptionAmendment::Table)
                    .col(DraftPickOptionAmendment::AmendmentType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("draft_pick_option_amendment_fk_option")
                    .from(
                        DraftPickOptionAmendment::Table,
                        DraftPickOptionAmendment::DraftPickOptionId,
                    )
                    .to(DraftPickOption::Table, DraftPickOption::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(DraftPickOptionAmendment::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
pub enum DraftPickOptionAmendment {
    Table,
    Id,
    AmendedClause,
    AmendmentType,
    DraftPickOptionId,
    CreatedAt,
    UpdatedAt,
}
