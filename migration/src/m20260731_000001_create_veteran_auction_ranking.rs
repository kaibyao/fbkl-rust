//! The season's ranked veteran-auction nomination list (rules §6.3.2, §6.3.6).
//!
//! Commissioner input, entered per season before the auction starts, one row per ranked player.
//! Pool assembly reads it to order releases and to spread ranked players across the configured
//! minimum-bid tiers; a pooled player who is absent from it is open-nomination at the bottom tier.

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
                    .table(VeteranAuctionRanking::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(VeteranAuctionRanking::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(VeteranAuctionRanking::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VeteranAuctionRanking::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VeteranAuctionRanking::PlayerId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VeteranAuctionRanking::NominationRank)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VeteranAuctionRanking::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(VeteranAuctionRanking::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, VeteranAuctionRanking::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("veteran_auction_ranking_fk_league")
                    .from(
                        VeteranAuctionRanking::Table,
                        VeteranAuctionRanking::LeagueId,
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
                    .name("veteran_auction_ranking_fk_player")
                    .from(
                        VeteranAuctionRanking::Table,
                        VeteranAuctionRanking::PlayerId,
                    )
                    .to(Player::Table, Player::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // A player is ranked at most once per league season.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("veteran_auction_ranking_league_season_player")
                    .table(VeteranAuctionRanking::Table)
                    .col(VeteranAuctionRanking::LeagueId)
                    .col(VeteranAuctionRanking::EndOfSeasonYear)
                    .col(VeteranAuctionRanking::PlayerId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Two players cannot share a nomination rank.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("veteran_auction_ranking_league_season_rank")
                    .table(VeteranAuctionRanking::Table)
                    .col(VeteranAuctionRanking::LeagueId)
                    .col(VeteranAuctionRanking::EndOfSeasonYear)
                    .col(VeteranAuctionRanking::NominationRank)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(VeteranAuctionRanking::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum VeteranAuctionRanking {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    PlayerId,
    NominationRank,
    CreatedAt,
    UpdatedAt,
}
