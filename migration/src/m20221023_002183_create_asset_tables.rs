use fbkl_entity::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};
use sea_orm_migration::prelude::*;

use crate::{
    m20220922_012310_create_real_world_tables::Player,
    m20220924_004529_create_league_tables::{League, LeaguePlayer, Team},
    set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;

        setup_contract(manager).await?;
        setup_draft_pick(manager).await?;
        setup_draft_pick_option(manager).await?;

        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(DraftPickOption::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(DraftPick::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Contract::Table).if_exists().to_owned())
            .await
    }
}

async fn setup_contract(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Contract::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Contract::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Contract::ContractYear)
                        .small_integer()
                        .not_null()
                        .default(1),
                )
                .col(
                    ColumnDef::new(Contract::ContractType)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Contract::IsIR)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(
                    ColumnDef::new(Contract::Salary)
                        .small_integer()
                        .not_null()
                        .default(1),
                )
                .col(
                    ColumnDef::new(Contract::EndOfSeasonYear)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Contract::Status)
                        .small_integer()
                        .not_null()
                        .default(0),
                )
                .col(ColumnDef::new(Contract::LeagueId).big_integer().not_null())
                .col(ColumnDef::new(Contract::PlayerId).big_integer())
                .col(ColumnDef::new(Contract::LeaguePlayerId).big_integer())
                .col(ColumnDef::new(Contract::PreviousContractId).big_integer())
                .col(ColumnDef::new(Contract::OriginalContractId).big_integer())
                .col(ColumnDef::new(Contract::TeamId).big_integer())
                .col(
                    ColumnDef::new(Contract::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(Contract::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, Contract::Table.to_string()).await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("contract_league_year_roster_player")
                .table(Contract::Table)
                .col(Contract::LeagueId)
                .col(Contract::EndOfSeasonYear)
                .col(Contract::TeamId)
                .col(Contract::PlayerId)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("contract_related_contracts")
                .table(Contract::Table)
                .col(Contract::OriginalContractId)
                .col(Contract::PreviousContractId)
                .to_owned(),
        )
        .await?;

    // Only 1 active contract per player is allowed for any given league & season
    manager
        .get_connection()
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "CREATE UNIQUE INDEX contract_unique_active_contract_per_player_per_league ON contract (league_id, end_of_season_year, player_id, status) WHERE status = 0".to_string(),
        ))
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("contract_fk_league")
                .from(Contract::Table, Contract::LeagueId)
                .to(League::Table, League::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("contract_fk_original_contract")
                .from(Contract::Table, Contract::OriginalContractId)
                .to(Contract::Table, Contract::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("contract_fk_previous_contract")
                .from(Contract::Table, Contract::PreviousContractId)
                .to(Contract::Table, Contract::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("contract_fk_player")
                .from(Contract::Table, Contract::PlayerId)
                .to(Player::Table, Player::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("contract_fk_league_player")
                .from(Contract::Table, Contract::LeaguePlayerId)
                .to(LeaguePlayer::Table, LeaguePlayer::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("contract_fk_team")
                .from(Contract::Table, Contract::TeamId)
                .to(Team::Table, Team::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await
}

async fn setup_draft_pick(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(DraftPick::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(DraftPick::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(ColumnDef::new(DraftPick::Round).small_integer().not_null())
                .col(
                    ColumnDef::new(DraftPick::EndOfSeasonYear)
                        .small_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(DraftPick::ProtectionClause).string())
                .col(ColumnDef::new(DraftPick::LeagueId).big_integer().not_null())
                .col(
                    ColumnDef::new(DraftPick::CurrentOwnerTeamId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DraftPick::OriginalOwnerTeamId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DraftPick::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .col(
                    ColumnDef::new(DraftPick::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                )
                .to_owned(),
        )
        .await?;

    set_auto_updated_at_on_table(manager, DraftPick::Table.to_string()).await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("draft_pick_league_year")
                .table(DraftPick::Table)
                .col(DraftPick::LeagueId)
                .col(DraftPick::EndOfSeasonYear)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("draft_pick_owner_teams")
                .table(DraftPick::Table)
                .col(DraftPick::OriginalOwnerTeamId)
                .col(DraftPick::CurrentOwnerTeamId)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("draft_pick_unique_picks_per_team")
                .table(DraftPick::Table)
                .unique()
                .col(DraftPick::EndOfSeasonYear)
                .col(DraftPick::Round)
                .col(DraftPick::OriginalOwnerTeamId)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("draft_pick_fk_league")
                .from(DraftPick::Table, DraftPick::LeagueId)
                .to(League::Table, League::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("draft_pick_fk_current_team")
                .from(DraftPick::Table, DraftPick::CurrentOwnerTeamId)
                .to(Team::Table, Team::Id)
                .on_delete(ForeignKeyAction::NoAction)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("draft_pick_fk_original_team")
                .from(DraftPick::Table, DraftPick::OriginalOwnerTeamId)
                .to(Team::Table, Team::Id)
                .on_delete(ForeignKeyAction::NoAction)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await
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
                    ColumnDef::new(DraftPickOption::DraftPickId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DraftPickOption::Status)
                        .small_integer()
                        .not_null()
                        .default(0),
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

    set_auto_updated_at_on_table(manager, DraftPickOption::Table.to_string()).await?;

    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("draft_pick_option_fk_draft_pick")
                .from(DraftPickOption::Table, DraftPickOption::DraftPickId)
                .to(DraftPick::Table, DraftPick::Id)
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            IndexCreateStatement::new()
                .name("draft_pick_option_draft_pick")
                .table(DraftPickOption::Table)
                .col(DraftPickOption::DraftPickId)
                .to_owned(),
        )
        .await
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum Contract {
    Table,
    Id,
    ContractYear,
    ContractType,
    IsIR,
    Salary,
    EndOfSeasonYear,
    Status,
    LeagueId,
    PlayerId,
    LeaguePlayerId,
    PreviousContractId,
    OriginalContractId,
    TeamId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum DraftPick {
    Table,
    Id,
    Round,
    EndOfSeasonYear,
    ProtectionClause,
    LeagueId,
    CurrentOwnerTeamId,
    OriginalOwnerTeamId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum DraftPickOption {
    Table,
    Id,
    Clause,
    Status,
    DraftPickId,
    CreatedAt,
    UpdatedAt,
}
