//! One row per team per season holding the standings inputs the rookie draft consumes (rules §7.2).
//!
//! FBKL cannot compute these itself — they come from the external league host, entered by the
//! commissioner. `mid_season_rank` is the ≈2/3-season snapshot frozen at a set week (§7.2.3) that
//! drives lottery odds and the rounds 2–5 order; `regular_season_rank` is the final standings, used
//! only as a tie-break; `playoff_finish` orders the playoff teams. Deliberately immutable stored
//! data, never recomputed from a live feed.

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
        manager
            .create_table(
                Table::create()
                    .table(LeagueTeamSeasonStanding::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::TeamId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::RegularSeasonRank)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::MidSeasonRank)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::MadePlayoffs)
                            .boolean()
                            .not_null(),
                    )
                    .col(ColumnDef::new(LeagueTeamSeasonStanding::PlayoffFinish).small_integer())
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(LeagueTeamSeasonStanding::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, LeagueTeamSeasonStanding::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("league_team_season_standing_fk_league")
                    .from(
                        LeagueTeamSeasonStanding::Table,
                        LeagueTeamSeasonStanding::LeagueId,
                    )
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("league_team_season_standing_fk_team")
                    .from(
                        LeagueTeamSeasonStanding::Table,
                        LeagueTeamSeasonStanding::TeamId,
                    )
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // A team has at most one standings row per season.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("league_team_season_standing_league_season_team")
                    .table(LeagueTeamSeasonStanding::Table)
                    .col(LeagueTeamSeasonStanding::LeagueId)
                    .col(LeagueTeamSeasonStanding::EndOfSeasonYear)
                    .col(LeagueTeamSeasonStanding::TeamId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(LeagueTeamSeasonStanding::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum LeagueTeamSeasonStanding {
    Table,
    Id,
    LeagueId,
    TeamId,
    EndOfSeasonYear,
    RegularSeasonRank,
    MidSeasonRank,
    MadePlayoffs,
    PlayoffFinish,
    CreatedAt,
    UpdatedAt,
}
