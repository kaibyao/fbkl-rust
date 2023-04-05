use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::Team, m20221117_235325_create_transaction::Transaction,
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
                    .table(TeamUpdate::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TeamUpdate::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TeamUpdate::UpdateType)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(TeamUpdate::Data).binary().not_null())
                    .col(ColumnDef::new(TeamUpdate::EffectiveDate).date().not_null())
                    .col(
                        ColumnDef::new(TeamUpdate::Status)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(TeamUpdate::TeamId).big_integer().not_null())
                    .col(ColumnDef::new(TeamUpdate::TransactionId).big_integer())
                    .col(
                        ColumnDef::new(TeamUpdate::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(TeamUpdate::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, TeamUpdate::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("team_update_fk_team")
                    .from(TeamUpdate::Table, TeamUpdate::TeamId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("team_update_fk_transaction")
                    .from(TeamUpdate::Table, TeamUpdate::TransactionId)
                    .to(Transaction::Table, Transaction::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("team_update_team_id")
                    .table(TeamUpdate::Table)
                    .col(TeamUpdate::TeamId)
                    .col(TeamUpdate::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("team_update_transaction")
                    .table(TeamUpdate::Table)
                    .col(TeamUpdate::TransactionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("team_update_type")
                    .table(TeamUpdate::Table)
                    .col(TeamUpdate::UpdateType)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(TeamUpdate::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum TeamUpdate {
    Table,
    Id,
    UpdateType,
    Data,
    EffectiveDate,
    Status,
    TeamId,
    TransactionId,
    CreatedAt,
    UpdatedAt,
}
