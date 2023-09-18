use fbkl_entity::sea_orm::TransactionTrait;
use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::{League, Team},
    set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;

        setup_draft_pick(manager).await?;

        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DraftPick::Table).if_exists().to_owned())
            .await
    }
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

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum DraftPick {
    Table,
    Id,
    Round,
    EndOfSeasonYear,
    LeagueId,
    CurrentOwnerTeamId,
    OriginalOwnerTeamId,
    CreatedAt,
    UpdatedAt,
}
