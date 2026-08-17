//! State for the two-stage RFA handshake and the pick it can cost (rules §15.2, §15.3).
//!
//! `rfa_resolution` is the row a closed RFA auction parks against: it remembers the keeper-deadline
//! owner (who holds the discount right even after the contract changes hands), the winning bid and
//! when it was announced, and the two 48h deadlines the scheduler fires on. One row per designated
//! RFA contract, so re-running designation cannot fork a player's resolution.
//!
//! `rfa_compensation_pick` records the pick a declining original owner is owed (§15.2.1 tier) and,
//! once the winner chooses it, which pick paid it. The pick itself moves by rewriting
//! `draft_pick.current_owner_team_id`, so `draft_pick` needs no schema change.

use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::{League, Team},
    m20221023_002183_create_contract::Contract,
    m20221023_002184_create_draft_pick::DraftPick,
    m20221112_132607_create_auction_tables::Auction,
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
                    .table(RfaResolution::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RfaResolution::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RfaResolution::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RfaResolution::EndOfSeasonYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RfaResolution::RfaContractId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RfaResolution::OriginalOwnerTeamId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RfaResolution::AuctionId).big_integer())
                    .col(ColumnDef::new(RfaResolution::WinningTeamId).big_integer())
                    .col(ColumnDef::new(RfaResolution::FinalBid).small_integer())
                    .col(ColumnDef::new(RfaResolution::FinalBidAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(RfaResolution::Status).string().not_null())
                    .col(ColumnDef::new(RfaResolution::RaisedBid).small_integer())
                    .col(
                        ColumnDef::new(RfaResolution::RaiseDeadlineAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RfaResolution::MatchDeadlineAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(RfaResolution::ResolvedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(RfaResolution::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(RfaResolution::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, RfaResolution::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_resolution_fk_league")
                    .from(RfaResolution::Table, RfaResolution::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_resolution_fk_contract")
                    .from(RfaResolution::Table, RfaResolution::RfaContractId)
                    .to(Contract::Table, Contract::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_resolution_fk_original_owner_team")
                    .from(RfaResolution::Table, RfaResolution::OriginalOwnerTeamId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_resolution_fk_auction")
                    .from(RfaResolution::Table, RfaResolution::AuctionId)
                    .to(Auction::Table, Auction::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_resolution_fk_winning_team")
                    .from(RfaResolution::Table, RfaResolution::WinningTeamId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // One resolution per designated RFA contract.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rfa_resolution_contract")
                    .table(RfaResolution::Table)
                    .col(RfaResolution::RfaContractId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RfaCompensationPick::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RfaCompensationPick::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RfaCompensationPick::RfaResolutionId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RfaCompensationPick::RequiredRound)
                            .small_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RfaCompensationPick::ForfeitedDraftPickId).big_integer())
                    .col(
                        ColumnDef::new(RfaCompensationPick::ToTeamId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RfaCompensationPick::FromTeamId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RfaCompensationPick::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(RfaCompensationPick::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, RfaCompensationPick::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_compensation_pick_fk_resolution")
                    .from(
                        RfaCompensationPick::Table,
                        RfaCompensationPick::RfaResolutionId,
                    )
                    .to(RfaResolution::Table, RfaResolution::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_compensation_pick_fk_draft_pick")
                    .from(
                        RfaCompensationPick::Table,
                        RfaCompensationPick::ForfeitedDraftPickId,
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
                    .name("rfa_compensation_pick_fk_to_team")
                    .from(RfaCompensationPick::Table, RfaCompensationPick::ToTeamId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("rfa_compensation_pick_fk_from_team")
                    .from(RfaCompensationPick::Table, RfaCompensationPick::FromTeamId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // A decline costs one pick, so a resolution owes at most one compensation row.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("rfa_compensation_pick_resolution")
                    .table(RfaCompensationPick::Table)
                    .col(RfaCompensationPick::RfaResolutionId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RfaCompensationPick::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(RfaResolution::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum RfaResolution {
    Table,
    Id,
    LeagueId,
    EndOfSeasonYear,
    RfaContractId,
    OriginalOwnerTeamId,
    AuctionId,
    WinningTeamId,
    FinalBid,
    FinalBidAt,
    Status,
    RaisedBid,
    RaiseDeadlineAt,
    MatchDeadlineAt,
    ResolvedAt,
    CreatedAt,
    UpdatedAt,
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum RfaCompensationPick {
    Table,
    Id,
    RfaResolutionId,
    RequiredRound,
    ForfeitedDraftPickId,
    ToTeamId,
    FromTeamId,
    CreatedAt,
    UpdatedAt,
}
