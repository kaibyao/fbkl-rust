//! Lottery audit storage plus the pick-owner denormalization the live rookie draft needs (rules §7.2).
//!
//! `rookie_draft_selection.current_owner_team_id` is copied from `draft_pick` when the slate is
//! built, so a traded pick is made by the acquirer without re-joining `draft_pick` on every board
//! render. Existing (imported) rows are backfilled from their draft pick.
//!
//! `rookie_draft_lottery` stores the RNG seed and a human-readable draw log so the six drawn
//! first-round slots can be re-verified independently; `rookie_draft_lottery_pick` stores each
//! drawn slot with the ball count that won it. Picks 7–12 are deterministic from `playoff_finish`
//! and need no storage.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{DatabaseBackend, Statement},
};

use crate::{
    m20220924_004529_create_league_tables::{League, Team},
    set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

async fn run_sql(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map(|_| ())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(
            manager,
            "ALTER TABLE rookie_draft_selection ADD COLUMN current_owner_team_id BIGINT",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE rookie_draft_selection AS rds
                SET current_owner_team_id = dp.current_owner_team_id
                FROM draft_pick AS dp
                WHERE dp.id = rds.draft_pick_id",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE rookie_draft_selection
                ALTER COLUMN current_owner_team_id SET NOT NULL,
                ADD CONSTRAINT rookie_draft_selection_fk_current_owner_team
                    FOREIGN KEY (current_owner_team_id) REFERENCES team(id)
                    ON UPDATE CASCADE ON DELETE NO ACTION",
        )
        .await?;

        manager
            .create_table(
                Table::create()
                    .table(RookieDraftLottery::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RookieDraftLottery::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLottery::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLottery::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLottery::RngSeed)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RookieDraftLottery::RngLog).text())
                    .col(
                        ColumnDef::new(RookieDraftLottery::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLottery::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, RookieDraftLottery::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rookie_draft_lottery_fk_league")
                    .from(RookieDraftLottery::Table, RookieDraftLottery::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // One lottery per league season — the unique index is what makes re-running it a no-op.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rookie_draft_lottery_league_season")
                    .table(RookieDraftLottery::Table)
                    .col(RookieDraftLottery::LeagueId)
                    .col(RookieDraftLottery::EndOfSeasonYear)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RookieDraftLotteryPick::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RookieDraftLotteryPick::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLotteryPick::RookieDraftLotteryId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLotteryPick::PickNumber)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLotteryPick::TeamId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLotteryPick::BallsHeld)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLotteryPick::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(RookieDraftLotteryPick::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, RookieDraftLotteryPick::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rookie_draft_lottery_pick_fk_lottery")
                    .from(
                        RookieDraftLotteryPick::Table,
                        RookieDraftLotteryPick::RookieDraftLotteryId,
                    )
                    .to(RookieDraftLottery::Table, RookieDraftLottery::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rookie_draft_lottery_pick_fk_team")
                    .from(
                        RookieDraftLotteryPick::Table,
                        RookieDraftLotteryPick::TeamId,
                    )
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rookie_draft_lottery_pick_lottery_pick_number")
                    .table(RookieDraftLotteryPick::Table)
                    .col(RookieDraftLotteryPick::RookieDraftLotteryId)
                    .col(RookieDraftLotteryPick::PickNumber)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RookieDraftLotteryPick::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(RookieDraftLottery::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        run_sql(
            manager,
            "ALTER TABLE rookie_draft_selection DROP COLUMN current_owner_team_id",
        )
        .await
    }
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum RookieDraftLottery {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    RngSeed,
    RngLog,
    CreatedAt,
    UpdatedAt,
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum RookieDraftLotteryPick {
    Table,
    Id,
    RookieDraftLotteryId,
    PickNumber,
    TeamId,
    BallsHeld,
    CreatedAt,
    UpdatedAt,
}
