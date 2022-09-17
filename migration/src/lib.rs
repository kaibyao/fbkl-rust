pub use sea_orm_migration::prelude::*;

mod m20220916_131202_create_user_table;
mod m20220916_152433_create_user_registration;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220916_131202_create_user_table::Migration),
            Box::new(m20220916_152433_create_user_registration::Migration),
        ]
    }
}
