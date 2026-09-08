//! The drops an owner submits with a trade so the trade fits their roster (rules §12.5.3, §13.1.4).
//!
//! A trade and one owner's accommodating drops are one transaction, so the drops have to be on
//! record when the trade processes. The proposer's arrive with the proposal and each other owner's
//! with their accept, which is why they are stored per team rather than per trade.

use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::Team, m20221023_002183_create_contract::Contract,
    m20221112_151717_create_trade_tables::Trade,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TradeAccommodatingDrop::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TradeAccommodatingDrop::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TradeAccommodatingDrop::TradeId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeAccommodatingDrop::TeamId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeAccommodatingDrop::ContractId)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("trade_accommodating_drop_fk_trade")
                    .from(
                        TradeAccommodatingDrop::Table,
                        TradeAccommodatingDrop::TradeId,
                    )
                    .to(Trade::Table, Trade::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("trade_accommodating_drop_fk_team")
                    .from(
                        TradeAccommodatingDrop::Table,
                        TradeAccommodatingDrop::TeamId,
                    )
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("trade_accommodating_drop_fk_contract")
                    .from(
                        TradeAccommodatingDrop::Table,
                        TradeAccommodatingDrop::ContractId,
                    )
                    .to(Contract::Table, Contract::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // One contract can be dropped for a trade at most once, whoever submitted it.
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("trade_accommodating_drop_trade_contract")
                    .table(TradeAccommodatingDrop::Table)
                    .col(TradeAccommodatingDrop::TradeId)
                    .col(TradeAccommodatingDrop::ContractId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(TradeAccommodatingDrop::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at <https://docs.rs/sea-query#iden>
#[derive(Iden)]
pub enum TradeAccommodatingDrop {
    Table,
    Id,
    TradeId,
    TeamId,
    ContractId,
}
