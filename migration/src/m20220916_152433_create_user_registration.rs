use sea_orm_migration::prelude::*;

use crate::{m20220916_131202_create_user_table::User, set_auto_updated_at_on_table};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserRegistration::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserRegistration::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserRegistration::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserRegistration::Token).binary().not_null())
                    .col(
                        ColumnDef::new(UserRegistration::Status)
                            .small_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(UserRegistration::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .col(
                        ColumnDef::new(UserRegistration::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .extra("DEFAULT CURRENT_TIMESTAMP".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        set_auto_updated_at_on_table(manager, UserRegistration::Table.to_string()).await?;

        manager
            .create_foreign_key(
                ForeignKeyCreateStatement::new()
                    .from(UserRegistration::Table, UserRegistration::UserId)
                    .to(User::Table, User::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .unique()
                    .name("user_registration_user_id")
                    .table(UserRegistration::Table)
                    .col(UserRegistration::UserId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserRegistration::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum UserRegistration {
    Table,
    Id,
    UserId,
    Token,
    Status,
    CreatedAt,
    UpdatedAt,
}
