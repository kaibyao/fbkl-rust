use sea_orm_migration::prelude::*;

use crate::{
    m20221112_151717_create_trade_tables::TradeAsset,
    m20230811_152800_create_draft_pick_option_amendment::DraftPickOptionAmendment,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(TradeAsset::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("draft_pick_option_amendment_id")).big_integer(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("trade_asset_draft_pick_option_amendment")
                    .table(TradeAsset::Table)
                    .col(TradeAsset::DraftPickOptionAmendmentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("trade_asset_fk_draft_pick_option_amendment")
                    .from(TradeAsset::Table, TradeAsset::DraftPickOptionAmendmentId)
                    .to(
                        DraftPickOptionAmendment::Table,
                        DraftPickOptionAmendment::Id,
                    )
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(TradeAsset::Table)
                    .drop_column(Alias::new("draft_pick_option_amendment_id"))
                    .to_owned(),
            )
            .await
    }
}
