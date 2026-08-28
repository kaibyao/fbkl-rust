//! The roster rules each team broke at a roster-lock deadline (rules §13.1.2, §13.2).
//!
//! Roster lock leaves an illegal team's `team_updates` Pending and records its broken rules here,
//! so the commissioner can read them from the API instead of the scheduler's logs. Each lock run
//! replaces the deadline's rows, so the table always shows the latest run's verdict.

use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::{League, Team},
    m20221117_235325_create_transaction::Deadline,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RosterLockViolation::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RosterLockViolation::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RosterLockViolation::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RosterLockViolation::DeadlineId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RosterLockViolation::TeamId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RosterLockViolation::Rule)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RosterLockViolation::Message)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RosterLockViolation::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("roster_lock_violation_fk_league")
                    .from(RosterLockViolation::Table, RosterLockViolation::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("roster_lock_violation_fk_deadline")
                    .from(RosterLockViolation::Table, RosterLockViolation::DeadlineId)
                    .to(Deadline::Table, Deadline::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("roster_lock_violation_fk_team")
                    .from(RosterLockViolation::Table, RosterLockViolation::TeamId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // A team breaks a given rule at most once per deadline, so a re-run cannot double-report it.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("roster_lock_violation_deadline_team_rule")
                    .table(RosterLockViolation::Table)
                    .col(RosterLockViolation::DeadlineId)
                    .col(RosterLockViolation::TeamId)
                    .col(RosterLockViolation::Rule)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // The commissioner's read path is "this league's violations".
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("roster_lock_violation_league")
                    .table(RosterLockViolation::Table)
                    .col(RosterLockViolation::LeagueId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RosterLockViolation::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum RosterLockViolation {
    Table,
    Id,
    LeagueId,
    DeadlineId,
    TeamId,
    Rule,
    Message,
    CreatedAt,
}
