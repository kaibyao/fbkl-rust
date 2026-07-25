//! The veteran auction's release schedule and its minimum-bid tiers (rules §6.3).
//!
//! `auction_schedule` holds one row per pooled player, built at keeper deadline; the daily release
//! tick opens auctions for rows whose date has arrived. `min_bid_tier_config` is the per-season
//! ordered tier list (§6.3.6) that supplies each auction's opening minimum bid and the single step
//! an unbid auction slides down to (§6.3.4).

use sea_orm_migration::prelude::*;

use crate::{
    m20220922_012310_create_real_world_tables::Player,
    m20220924_004529_create_league_tables::League, set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuctionSchedule::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuctionSchedule::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AuctionSchedule::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuctionSchedule::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuctionSchedule::PlayerId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuctionSchedule::ScheduledReleaseDate)
                            .date()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuctionSchedule::NominationRank).small_integer())
                    .col(
                        ColumnDef::new(AuctionSchedule::MinBidTier)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuctionSchedule::IsRfaWeek)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(AuctionSchedule::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(AuctionSchedule::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, AuctionSchedule::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("auction_schedule_fk_league")
                    .from(AuctionSchedule::Table, AuctionSchedule::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("auction_schedule_fk_player")
                    .from(AuctionSchedule::Table, AuctionSchedule::PlayerId)
                    .to(Player::Table, Player::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // A player is pooled at most once per league season.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("auction_schedule_league_season_player")
                    .table(AuctionSchedule::Table)
                    .col(AuctionSchedule::LeagueId)
                    .col(AuctionSchedule::EndOfSeasonYear)
                    .col(AuctionSchedule::PlayerId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("auction_schedule_league_season_release_date")
                    .table(AuctionSchedule::Table)
                    .col(AuctionSchedule::LeagueId)
                    .col(AuctionSchedule::EndOfSeasonYear)
                    .col(AuctionSchedule::ScheduledReleaseDate)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MinBidTierConfig::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MinBidTierConfig::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MinBidTierConfig::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MinBidTierConfig::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MinBidTierConfig::TierIndex)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MinBidTierConfig::MinBidAmount)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MinBidTierConfig::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(MinBidTierConfig::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, MinBidTierConfig::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("min_bid_tier_config_fk_league")
                    .from(MinBidTierConfig::Table, MinBidTierConfig::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("min_bid_tier_config_league_season_tier")
                    .table(MinBidTierConfig::Table)
                    .col(MinBidTierConfig::LeagueId)
                    .col(MinBidTierConfig::EndOfSeasonYear)
                    .col(MinBidTierConfig::TierIndex)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MinBidTierConfig::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(AuctionSchedule::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum AuctionSchedule {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    PlayerId,
    ScheduledReleaseDate,
    NominationRank,
    MinBidTier,
    IsRfaWeek,
    CreatedAt,
    UpdatedAt,
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum MinBidTierConfig {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    TierIndex,
    MinBidAmount,
    CreatedAt,
    UpdatedAt,
}
