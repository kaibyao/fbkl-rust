use sea_orm_migration::prelude::*;

use crate::{
    m20221023_002183_create_asset_tables::Contract,
    m20230217_011454_create_team_update::TeamUpdate, set_auto_updated_at_on_table,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TeamUpdateContract::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TeamUpdateContract::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TeamUpdateContract::UpdateType)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TeamUpdateContract::TeamUpdateId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TeamUpdateContract::ContractId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TeamUpdateContract::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(TeamUpdateContract::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, TeamUpdateContract::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("team_update_contract_fk_team_update")
                    .from(TeamUpdateContract::Table, TeamUpdateContract::TeamUpdateId)
                    .to(TeamUpdate::Table, TeamUpdate::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("team_update_contract_fk_contract")
                    .from(TeamUpdateContract::Table, TeamUpdateContract::ContractId)
                    .to(Contract::Table, Contract::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("team_update_contract_ids")
                    .table(TeamUpdateContract::Table)
                    .col(TeamUpdateContract::TeamUpdateId)
                    .col(TeamUpdateContract::ContractId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(TeamUpdateContract::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum TeamUpdateContract {
    Table,
    Id,
    UpdateType,
    TeamUpdateId,
    ContractId,
    CreatedAt,
    UpdatedAt,
}
