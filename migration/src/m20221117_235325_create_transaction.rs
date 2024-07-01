use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::League, m20221023_002183_create_contract::Contract,
    m20221111_002318_create_rookie_draft::RookieDraftSelection,
    m20221112_132607_create_auction_tables::Auction, m20221112_151717_create_trade_tables::Trade,
    set_auto_updated_at_on_table,
};

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
            .await?;

        manager
            .drop_table(Table::drop().table(Deadline::Table).if_exists().to_owned())
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
                .col(ColumnDef::new(Deadline::Kind).string().not_null())
                .col(ColumnDef::new(Deadline::Name).string().not_null())
                .col(
                    ColumnDef::new(Deadline::EndOfSeasonYear)
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
                .col(Deadline::EndOfSeasonYear)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("deadline_kind")
                .table(Deadline::Table)
                .col(Deadline::Kind)
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
                    ColumnDef::new(Transaction::EndOfSeasonYear)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Transaction::LeagueId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Transaction::Kind)
                        .string()
                        .not_null(),
                )
                .col(ColumnDef::new(Transaction::AuctionId).big_integer())
                .col(
                    ColumnDef::new(Transaction::DeadlineId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Transaction::DroppedContractId).big_integer())
                .col(ColumnDef::new(Transaction::IrContractId).big_integer())
                .col(ColumnDef::new(Transaction::RdiContractId).big_integer())
                .col(ColumnDef::new(Transaction::RookieContractActivationId).big_integer())
                .col(ColumnDef::new(Transaction::RookieDraftSelectionId).big_integer())
                .col(ColumnDef::new(Transaction::TradeId).big_integer())
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
                .name("transaction_fk_auction")
                .from(Transaction::Table, Transaction::AuctionId)
                .to(Auction::Table, Auction::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("transaction_fk_deadline")
                .from(Transaction::Table, Transaction::DeadlineId)
                .to(Deadline::Table, Deadline::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("transaction_fk_dropped_contract")
                .from(Transaction::Table, Transaction::DroppedContractId)
                .to(Contract::Table, Contract::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
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
        .create_foreign_key(
            ForeignKey::create()
                .name("transaction_fk_rookie_draft_selection")
                .from(Transaction::Table, Transaction::RookieDraftSelectionId)
                .to(RookieDraftSelection::Table, RookieDraftSelection::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("transaction_fk_trade")
                .from(Transaction::Table, Transaction::TradeId)
                .to(Trade::Table, Trade::Id)
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
                .col(Transaction::EndOfSeasonYear)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("transaction_deadline")
                .table(Transaction::Table)
                .col(Transaction::DeadlineId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("transaction_dropped_contract")
                .table(Transaction::Table)
                .col(Transaction::DroppedContractId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("transaction_kind")
                .table(Transaction::Table)
                .col(Transaction::Kind)
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
    Kind,
    Name,
    EndOfSeasonYear,
    LeagueId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum Transaction {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    Kind,
    AuctionId,
    DeadlineId,
    DroppedContractId,
    IrContractId,
    RdiContractId,
    RookieContractActivationId,
    RookieDraftSelectionId,
    TradeId,
    CreatedAt,
    UpdatedAt,
}
