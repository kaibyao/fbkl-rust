use sea_orm_migration::prelude::*;

use crate::{
    m20220922_012310_create_real_world_tables::Player,
    m20220924_004529_create_league_tables::League, m20221023_002183_create_asset_tables::DraftPick,
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
                        ColumnDef::new(RookieDraftSelection::SeasonEndYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftSelection::Status)
                            .small_integer()
                            .not_null()
                            .default(0),
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
                        ColumnDef::new(RookieDraftSelection::SelectedPlayerId)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rookie_draft_selection_year_league")
                    .table(RookieDraftSelection::Table)
                    .col(RookieDraftSelection::SeasonEndYear)
                    .col(RookieDraftSelection::LeagueId)
                    .col(RookieDraftSelection::Status)
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
            .create_index(
                IndexCreateStatement::new()
                    .name("rookie_draft_selection_unique_player")
                    .table(RookieDraftSelection::Table)
                    .col(RookieDraftSelection::SelectedPlayerId)
                    .unique()
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
                    .name("rookie_draft_selection_fk_selected_player")
                    .from(
                        RookieDraftSelection::Table,
                        RookieDraftSelection::SelectedPlayerId,
                    )
                    .to(Player::Table, Player::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RookieDraftSelection::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum RookieDraftSelection {
    Table,
    Id,
    Order,
    SeasonEndYear,
    Status,
    DraftPickId,
    LeagueId,
    SelectedPlayerId,
}
