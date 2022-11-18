use sea_orm_migration::prelude::*;

use crate::{m20220924_004529_create_league_tables::League, set_auto_updated_at_on_table};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Transaction::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Transaction::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Transaction::SeasonEndYear)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transaction::TransactionType)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transaction::LeagueId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Transaction::ReferredId)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, Transaction::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("transaction_fk_league")
                    .from(Transaction::Table, Transaction::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("transaction_league_year")
                    .table(Transaction::Table)
                    .col(Transaction::LeagueId)
                    .col(Transaction::SeasonEndYear)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("transaction_type_referred_id")
                    .table(Transaction::Table)
                    .col(Transaction::TransactionType)
                    .col(Transaction::ReferredId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Transaction::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Transaction {
    Table,
    Id,
    SeasonEndYear,
    TransactionType,
    LeagueId,
    ReferredId,
}
