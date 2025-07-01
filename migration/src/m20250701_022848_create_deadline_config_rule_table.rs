use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::League, m20221112_132607_create_auction_tables::Auction,
    m20221117_235325_create_transaction::Deadline, set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Deadline::Table)
                    .add_column_if_not_exists(ColumnDef::new(Deadline::Status).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("deadline_status")
                    .table(Deadline::Table)
                    .col(Deadline::Status)
                    .to_owned(),
            )
            .await?;

        // Create deadline_config_rules table
        manager
            .create_table(
                Table::create()
                    .table(DeadlineConfigRule::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DeadlineConfigRule::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DeadlineConfigRule::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeadlineConfigRule::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeadlineConfigRule::PreseasonKeeperDeadline)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            DeadlineConfigRule::VeteranAuctionDaysAfterKeeperDeadlineDuration,
                        )
                        .small_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeadlineConfigRule::FaAuctionDaysDuration)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            DeadlineConfigRule::FinalRosterLockDeadlineDaysAfterRookieDraft,
                        )
                        .small_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeadlineConfigRule::PlayoffsStartWeek)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeadlineConfigRule::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(DeadlineConfigRule::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, DeadlineConfigRule::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("deadline_config_rule_fk_league")
                    .from(DeadlineConfigRule::Table, DeadlineConfigRule::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // Add unique constraint for league_id and end_of_season_year
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("unique_deadline_config_rule_per_league_season")
                    .table(DeadlineConfigRule::Table)
                    .col(DeadlineConfigRule::LeagueId)
                    .col(DeadlineConfigRule::EndOfSeasonYear)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Add deadline_id column to auction table
        manager
            .alter_table(
                Table::alter()
                    .table(Auction::Table)
                    .add_column(ColumnDef::new(Auction::DeadlineId).big_integer().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("auction_fk_deadline")
                    .from(Auction::Table, Auction::DeadlineId)
                    .to(Deadline::Table, Deadline::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop deadline_id column from auction table
        manager
            .alter_table(
                Table::alter()
                    .table(Auction::Table)
                    .drop_column(Auction::DeadlineId)
                    .to_owned(),
            )
            .await?;

        // Drop deadline_config_rules table
        manager
            .drop_table(Table::drop().table(DeadlineConfigRule::Table).to_owned())
            .await?;

        // Drop status column from deadline table
        manager
            .alter_table(
                Table::alter()
                    .table(Deadline::Table)
                    .drop_column(Deadline::Status)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum DeadlineConfigRule {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    PreseasonKeeperDeadline,
    VeteranAuctionDaysAfterKeeperDeadlineDuration,
    FaAuctionDaysDuration,
    FinalRosterLockDeadlineDaysAfterRookieDraft,
    PlayoffsStartWeek,
    CreatedAt,
    UpdatedAt,
}
