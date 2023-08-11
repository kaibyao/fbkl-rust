pub use sea_orm_migration::MigratorTrait;
use sea_orm_migration::{
    async_trait,
    sea_orm::{ConnectionTrait, DatabaseBackend, Statement},
    DbErr, MigrationTrait, SchemaManager,
};

mod m20220916_131200_create_session_table;
mod m20220916_131201_create_auto_updated_at_fn;
mod m20220916_131202_create_user_table;
mod m20220916_152433_create_user_registration;
mod m20220922_012310_create_real_world_tables;
mod m20220924_004529_create_league_tables;
mod m20220930_011056_seed_positions;
mod m20221023_002183_create_asset_tables;
mod m20221111_002318_create_rookie_draft;
mod m20221112_132607_create_auction_tables;
mod m20221112_151717_create_trade_tables;
mod m20221117_235325_create_transaction;
mod m20230217_011454_create_team_update;
mod m20230811_152800_create_draft_pick_option_amendment;
mod m20230811_160328_add_draft_pick_option_amendment_fk_to_trade_asset;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220916_131200_create_session_table::Migration),
            Box::new(m20220916_131201_create_auto_updated_at_fn::Migration),
            Box::new(m20220916_131202_create_user_table::Migration),
            Box::new(m20220916_152433_create_user_registration::Migration),
            Box::new(m20220922_012310_create_real_world_tables::Migration),
            Box::new(m20220924_004529_create_league_tables::Migration),
            Box::new(m20220930_011056_seed_positions::Migration),
            Box::new(m20221023_002183_create_asset_tables::Migration),
            Box::new(m20221111_002318_create_rookie_draft::Migration),
            Box::new(m20221112_132607_create_auction_tables::Migration),
            Box::new(m20221112_151717_create_trade_tables::Migration),
            Box::new(m20221117_235325_create_transaction::Migration),
            Box::new(m20230217_011454_create_team_update::Migration),
            Box::new(m20230811_152800_create_draft_pick_option_amendment::Migration),
            Box::new(m20230811_160328_add_draft_pick_option_amendment_fk_to_trade_asset::Migration),
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
