use sea_orm_migration::prelude::*;

use crate::{
    m20220924_004529_create_league_tables::TeamUpdate,
    m20221023_002183_create_asset_tables::Contract,
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
                        ColumnDef::new(TeamUpdateContract::ActionType)
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
                    .to_owned(),
            )
            .await?;

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

// TODO: Do I use a separate table for storing team setting changes? If so, this table could really be called team_contract_update_action.
// Maybe team_update could have a "type" enum that determines whether the update is related to the roster vs team config.
// Then it can also store a comprehensive before vs after struct of the team w/ its settings + contracts.
// We also need a "date_active" for a team update so it only applies starting in the next week.

// Also TODO: Create entity for this table + relationships + inverse relationships

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
pub enum TeamUpdateContract {
    Table,
    Id,
    ActionType,
    TeamUpdateId,
    ContractId,
}
