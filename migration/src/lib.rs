use async_sea_orm_session::migration::SessionTableMigration;
pub use sea_orm_migration::MigratorTrait;
use sea_orm_migration::{
    async_trait,
    sea_orm::{ConnectionTrait, DatabaseBackend, Statement},
    DbErr, MigrationTrait, SchemaManager,
};

mod m20220916_131201_create_auto_updated_at_fn;
mod m20220916_131202_create_user_table;
mod m20220916_152433_create_user_registration;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(SessionTableMigration),
            Box::new(m20220916_131201_create_auto_updated_at_fn::Migration),
            Box::new(m20220916_131202_create_user_table::Migration),
            Box::new(m20220916_152433_create_user_registration::Migration),
        ]
    }
}

pub async fn set_auto_updated_at_on_table(
    manager: &SchemaManager<'_>,
    table: String,
) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT set_auto_updated_at_on_table('{}')", table),
        ))
        .await?;

    Ok(())
}