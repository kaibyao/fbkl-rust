use sea_orm_migration::prelude::*;

use crate::{
    m20220922_012310_create_real_world_tables::Position,
    m20220924_004529_create_league_tables::{League, Team, TeamUser},
    m20221023_002183_create_asset_tables::{Contract, DraftPick},
    set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        setup_trade(manager).await?;
        setup_trade_action(manager).await?;
        setup_trade_asset(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(TradeAsset::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(TradeAction::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Trade::Table).if_exists().to_owned())
            .await
    }
}

async fn setup_trade(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Trade::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Trade::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Trade::SeasonEndYear)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Trade::Status)
                        .small_integer()
                        .not_null()
                        .default(0),
                )
                .col(ColumnDef::new(Trade::LeagueId).big_integer().not_null())
                .col(ColumnDef::new(Trade::FromTeamId).big_integer().not_null())
                .col(ColumnDef::new(Trade::ToTeamId).big_integer().not_null())
                .col(ColumnDef::new(Trade::OriginalTradeId).big_integer())
                .col(ColumnDef::new(Trade::PreviousTradeId).big_integer())
                .col(
                    ColumnDef::new(Trade::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Trade::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Trade::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_fk_league")
                .from(Trade::Table, Trade::LeagueId)
                .to(League::Table, League::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_fk_from_team")
                .from(Trade::Table, Trade::FromTeamId)
                .to(Team::Table, Team::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_fk_to_team")
                .from(Trade::Table, Trade::FromTeamId)
                .to(Team::Table, Team::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_fk_original_trade")
                .from(Trade::Table, Trade::OriginalTradeId)
                .to(Trade::Table, Trade::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_fk_previous_trade")
                .from(Trade::Table, Trade::PreviousTradeId)
                .to(Trade::Table, Trade::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("trade_year_league")
                .table(Trade::Table)
                .col(Trade::SeasonEndYear)
                .col(Trade::LeagueId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("trade_related_trades")
                .table(Trade::Table)
                .col(Trade::OriginalTradeId)
                .col(Trade::PreviousTradeId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("trade_from_team")
                .table(Trade::Table)
                .col(Trade::FromTeamId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("trade_to_team")
                .table(Trade::Table)
                .col(Trade::ToTeamId)
                .to_owned(),
        )
        .await
}

async fn setup_trade_action(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(TradeAction::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(TradeAction::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(TradeAction::ActionType)
                        .small_integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(TradeAction::TeamUserId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(TradeAction::TradeId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(TradeAction::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(TradeAction::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, TradeAction::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_action_fk_team_user")
                .from(TradeAction::Table, TradeAction::TeamUserId)
                .to(TeamUser::Table, TeamUser::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_action_fk_trade")
                .from(TradeAction::Table, TradeAction::TradeId)
                .to(Trade::Table, Trade::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("trade_action_team_user")
                .table(TradeAction::Table)
                .col(TradeAction::TeamUserId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("trade_action_trade")
                .table(TradeAction::Table)
                .col(TradeAction::TradeId)
                .to_owned(),
        )
        .await
}

async fn setup_trade_asset(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(TradeAsset::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(TradeAsset::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(TradeAsset::AssetType)
                        .small_integer()
                        .not_null()
                        .default(0),
                )
                .col(ColumnDef::new(TradeAsset::DraftPickOption).string())
                .col(ColumnDef::new(TradeAsset::PlayerNameAtTimeOfTrade).string())
                .col(ColumnDef::new(TradeAsset::PlayerTeamNameAtTimeOfTrade).string())
                .col(ColumnDef::new(TradeAsset::ContractId).big_integer())
                .col(ColumnDef::new(TradeAsset::DraftPickId).big_integer())
                .col(
                    ColumnDef::new(TradeAsset::FromTeamId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(TradeAsset::PlayerPositionIdAtTimeOfTrade).big_integer())
                .col(ColumnDef::new(TradeAsset::TradeId).big_integer().not_null())
                .col(
                    ColumnDef::new(TradeAsset::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(TradeAsset::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, TradeAsset::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_asset_fk_contract")
                .from(TradeAsset::Table, TradeAsset::ContractId)
                .to(Contract::Table, Contract::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_asset_fk_draft_pick")
                .from(TradeAsset::Table, TradeAsset::DraftPickId)
                .to(DraftPick::Table, DraftPick::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_asset_fk_team")
                .from(TradeAsset::Table, TradeAsset::FromTeamId)
                .to(Team::Table, Team::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_asset_fk_position")
                .from(TradeAsset::Table, TradeAsset::PlayerPositionIdAtTimeOfTrade)
                .to(Position::Table, Position::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("trade_asset_fk_trade")
                .from(TradeAsset::Table, TradeAsset::TradeId)
                .to(Trade::Table, Trade::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("trade_asset_trade")
                .table(TradeAsset::Table)
                .col(TradeAsset::TradeId)
                .to_owned(),
        )
        .await
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum Trade {
    Table,
    Id,
    SeasonEndYear,
    Status,
    LeagueId,
    FromTeamId,
    ToTeamId,
    OriginalTradeId,
    PreviousTradeId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum TradeAction {
    Table,
    Id,
    ActionType,
    TeamUserId,
    TradeId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum TradeAsset {
    Table,
    Id,
    AssetType,
    DraftPickOption,
    PlayerNameAtTimeOfTrade,
    PlayerTeamNameAtTimeOfTrade,
    ContractId,
    DraftPickId,
    FromTeamId,
    PlayerPositionIdAtTimeOfTrade,
    TradeId,
    CreatedAt,
    UpdatedAt,
}
