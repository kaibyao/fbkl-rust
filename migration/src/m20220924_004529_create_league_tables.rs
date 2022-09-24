use sea_orm_migration::{prelude::*, sea_orm::TransactionTrait};

use crate::{m20220916_131202_create_user_table::User, set_auto_updated_at_on_table};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;

        manager
            .create_table(
                Table::create()
                    .table(League::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(League::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(League::Name).string().not_null())
                    .col(
                        ColumnDef::new(League::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(League::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, League::Table.to_string()).await?;

        manager
            .create_table(
                Table::create()
                    .table(Team::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Team::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Team::Name).string().not_null())
                    .col(ColumnDef::new(Team::LeagueId).big_integer().not_null())
                    .col(
                        ColumnDef::new(Team::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(Team::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, Team::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("team_fk_league")
                    .from(Team::Table, Team::LeagueId)
                    .to(League::Table, League::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("team_unique_league_id_and_name")
                    .table(Team::Table)
                    .unique()
                    .col(Team::LeagueId)
                    .col(Team::Name)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TeamUser::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TeamUser::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TeamUser::Nickname).string().not_null())
                    .col(ColumnDef::new(TeamUser::TeamId).big_integer().not_null())
                    .col(ColumnDef::new(TeamUser::UserId).big_integer().not_null())
                    .col(
                        ColumnDef::new(TeamUser::LeagueRole)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TeamUser::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(TeamUser::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, TeamUser::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("team_user_fk_team")
                    .from(TeamUser::Table, TeamUser::TeamId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("team_user_fk_user")
                    .from(TeamUser::Table, TeamUser::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // A user cannot own more than 1 team per league
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("team_user_unique")
                    .table(TeamUser::Table)
                    .unique()
                    .col(TeamUser::TeamId)
                    .col(TeamUser::UserId)
                    .to_owned(),
            )
            .await?;

        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TeamUser::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Team::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(League::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum League {
    Table,
    Id,
    Name,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Team {
    Table,
    Id,
    Name,
    LeagueId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum TeamUser {
    Table,
    Id,
    Nickname,
    TeamId,
    UserId,
    LeagueRole,
    CreatedAt,
    UpdatedAt,
}
