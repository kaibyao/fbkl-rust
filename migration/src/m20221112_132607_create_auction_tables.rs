use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::TeamUser, m20221023_002183_create_contract::Contract,
    set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        setup_auction(manager).await?;
        setup_auction_bid(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(AuctionBid::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().if_exists().table(Auction::Table).to_owned())
            .await
    }
}

async fn setup_auction(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Auction::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Auction::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Auction::Kind)
                        .string()
                        .not_null()
                        .default("FreeAgent"),
                )
                .col(ColumnDef::new(Auction::ContractId).big_integer().not_null())
                .col(
                    ColumnDef::new(Auction::MinimumBidAmount)
                        .small_integer()
                        .default(1),
                )
                .col(
                    ColumnDef::new(Auction::StartTimestamp)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Auction::SoftEndTimestamp)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP + INTERVAL '1 day'".to_string()),
                )
                .col(
                    ColumnDef::new(Auction::FixedEndTimestamp)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Auction::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Auction::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Auction::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("auction_fk_contract")
                .from(Auction::Table, Auction::ContractId)
                .to(Contract::Table, Contract::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("auction_contract")
                .table(Auction::Table)
                .col(Auction::ContractId)
                .to_owned(),
        )
        .await
}

async fn setup_auction_bid(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(AuctionBid::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(AuctionBid::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(AuctionBid::BidAmount)
                        .small_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(AuctionBid::Comment).string())
                .col(
                    ColumnDef::new(AuctionBid::AuctionId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(AuctionBid::TeamUserId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(AuctionBid::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(AuctionBid::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, AuctionBid::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("auction_bid_fk_auction")
                .from(AuctionBid::Table, AuctionBid::AuctionId)
                .to(Auction::Table, Auction::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("auction_bid_fk_team_user")
                .from(AuctionBid::Table, AuctionBid::TeamUserId)
                .to(TeamUser::Table, TeamUser::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("auction_bid_auction_bid_team_user")
                .table(AuctionBid::Table)
                .col(AuctionBid::AuctionId)
                .col(AuctionBid::BidAmount)
                .col(AuctionBid::TeamUserId)
                .to_owned(),
        )
        .await
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum Auction {
    Table,
    Id,
    Kind,
    MinimumBidAmount,
    StartTimestamp,
    SoftEndTimestamp,
    FixedEndTimestamp,
    ContractId,
    DeadlineId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum AuctionBid {
    Table,
    Id,
    BidAmount,
    Comment,
    AuctionId,
    TeamUserId,
    CreatedAt,
    UpdatedAt,
}
