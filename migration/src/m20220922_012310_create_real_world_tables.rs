use sea_orm_migration::{prelude::*, sea_orm::TransactionTrait};

use crate::set_auto_updated_at_on_table;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let transaction = db.begin().await?;

        setup_position(manager).await?;
        setup_real_team(manager).await?;
        setup_player(manager).await?;

        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Player::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RealTeam::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Position::Table).if_exists().to_owned())
            .await
    }
}

async fn setup_position(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Position::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Position::Id)
                        .integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(Position::Name).string().not_null())
                .col(ColumnDef::new(Position::EspnId).small_integer().not_null())
                .col(
                    ColumnDef::new(Position::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Position::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Position::Table.to_string()).await
}

async fn setup_real_team(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(RealTeam::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(RealTeam::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(RealTeam::City).string().not_null())
                .col(ColumnDef::new(RealTeam::Name).string().not_null())
                .col(ColumnDef::new(RealTeam::Code).string().not_null())
                .col(ColumnDef::new(RealTeam::EspnId).small_integer().not_null())
                .col(ColumnDef::new(RealTeam::NbaId).integer().not_null())
                .col(ColumnDef::new(RealTeam::LogoUrl).string().not_null())
                .col(
                    ColumnDef::new(RealTeam::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(RealTeam::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, RealTeam::Table.to_string()).await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("real_team_unique_code")
                .unique()
                .table(RealTeam::Table)
                .col(RealTeam::Code)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("real_team_unique_espn_id")
                .unique()
                .table(RealTeam::Table)
                .col(RealTeam::EspnId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("real_team_unique_nba_id")
                .unique()
                .table(RealTeam::Table)
                .col(RealTeam::NbaId)
                .to_owned(),
        )
        .await
}

async fn setup_player(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Player::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Player::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(Player::Name).string().not_null())
                .col(ColumnDef::new(Player::PositionId).integer().not_null())
                .col(
                    ColumnDef::new(Player::Status)
                        .string()
                        .not_null()
                        .default("Retired"),
                )
                .col(
                    ColumnDef::new(Player::CurrentRealTeamId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Player::IsRdiEligible)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(Player::PhotoUrl).string())
                .col(ColumnDef::new(Player::ThumbnailUrl).string())
                .col(ColumnDef::new(Player::EspnId).integer())
                .col(ColumnDef::new(Player::NbaId).integer())
                .col(
                    ColumnDef::new(Player::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Player::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Player::Table.to_string()).await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("player_real_team_id")
                .table(Player::Table)
                .col(Player::CurrentRealTeamId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("player_position_id")
                .table(Player::Table)
                .col(Player::PositionId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("player_espn_id")
                .table(Player::Table)
                .col(Player::EspnId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            IndexCreateStatement::new()
                .name("player_nba_id")
                .table(Player::Table)
                .col(Player::NbaId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("player_fk_position")
                .from(Player::Table, Player::PositionId)
                .to(Position::Table, Position::Id)
                .on_delete(ForeignKeyAction::NoAction)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("player_fk_real_team")
                .from(Player::Table, Player::CurrentRealTeamId)
                .to(RealTeam::Table, RealTeam::Id)
                .on_delete(ForeignKeyAction::NoAction)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum Position {
    Table,
    Id,
    Name,
    EspnId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum RealTeam {
    Table,
    Id,
    City,
    Name,
    Code,
    EspnId,
    NbaId,
    LogoUrl,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum Player {
    Table,
    Id,
    Name,
    PositionId,
    Status,
    CurrentRealTeamId,
    IsRdiEligible,
    PhotoUrl,
    ThumbnailUrl,
    EspnId,
    NbaId,
    CreatedAt,
    UpdatedAt,
}
