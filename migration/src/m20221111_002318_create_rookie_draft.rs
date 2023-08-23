use sea_orm_migration::prelude::*;

use crate::m20221023_002183_create_asset_tables::Contract;
use crate::{
    m20220924_004529_create_league_tables::League, m20221023_002183_create_asset_tables::DraftPick,
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
                    .table(RookieDraftSelection::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RookieDraftSelection::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RookieDraftSelection::Order).small_integer())
                    .col(
                        ColumnDef::new(RookieDraftSelection::Status)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RookieDraftSelection::ContractId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftSelection::DraftPickId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftSelection::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftSelection::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(RookieDraftSelection::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, RookieDraftSelection::Table.to_string()).await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rookie_draft_selection_year_league")
                    .table(RookieDraftSelection::Table)
                    .col(RookieDraftSelection::LeagueId)
                    .col(RookieDraftSelection::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rookie_draft_selection_unique_contract")
                    .table(RookieDraftSelection::Table)
                    .col(RookieDraftSelection::ContractId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rookie_draft_selection_unique_draft_pick")
                    .table(RookieDraftSelection::Table)
                    .col(RookieDraftSelection::DraftPickId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rookie_draft_selection_fk_contract")
                    .from(
                        RookieDraftSelection::Table,
                        RookieDraftSelection::ContractId,
                    )
                    .to(Contract::Table, Contract::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rookie_draft_selection_fk_draft_pick")
                    .from(
                        RookieDraftSelection::Table,
                        RookieDraftSelection::DraftPickId,
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
                    .name("rookie_draft_selection_fk_league")
                    .from(RookieDraftSelection::Table, RookieDraftSelection::LeagueId)
                    .to(League::Table, League::Id)
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
                    .table(RookieDraftSelection::Table)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum RookieDraftSelection {
    Table,
    Id,
    Order,
    Status,
    ContractId,
    DraftPickId,
    LeagueId,
    CreatedAt,
    UpdatedAt,
}
