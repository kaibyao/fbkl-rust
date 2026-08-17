// schema-DSL migration functions are naturally long and run once at startup
#![allow(clippy::too_many_lines, clippy::large_futures)]

pub use sea_orm_migration::MigratorTrait;
use sea_orm_migration::{
    DbErr, MigrationTrait, SchemaManager, async_trait,
    sea_orm::{ConnectionTrait, DatabaseBackend, Statement},
};

mod m20220916_131201_create_auto_updated_at_fn;
mod m20220916_131202_create_user_table;
mod m20220916_152433_create_user_registration;
mod m20220922_012310_create_real_world_tables;
mod m20220924_004529_create_league_tables;
mod m20220930_011056_seed_positions;
mod m20221023_002183_create_contract;
mod m20221023_002184_create_draft_pick;
mod m20221023_002185_create_draft_pick_option;
mod m20221111_002318_create_rookie_draft;
mod m20221112_132607_create_auction_tables;
mod m20221112_151717_create_trade_tables;
mod m20221117_235325_create_transaction;
mod m20230217_011454_create_team_update;
mod m20260609_000001_create_job_run;
mod m20260722_000001_collapse_transaction_fk_columns;
mod m20260725_000001_add_player_eligibility_fields;
mod m20260725_000002_alter_auction_add_status_and_owner;
mod m20260725_000003_create_auction_schedule_tables;
mod m20260730_000001_rename_auction_close_timestamps;
mod m20260731_000001_create_veteran_auction_ranking;
mod m20260731_000002_create_league_team_season_standing;
mod m20260731_000003_create_rookie_draft_lottery_tables;
mod m20260815_000001_create_rfa_resolution_tables;
mod m20260815_000002_alter_rfa_resolution_raise_deadline_nullable;
mod m20260817_000001_require_rfa_compensation_pick;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220916_131201_create_auto_updated_at_fn::Migration),
            Box::new(m20220916_131202_create_user_table::Migration),
            Box::new(m20220916_152433_create_user_registration::Migration),
            Box::new(m20220922_012310_create_real_world_tables::Migration),
            Box::new(m20220924_004529_create_league_tables::Migration),
            Box::new(m20220930_011056_seed_positions::Migration),
            Box::new(m20221023_002183_create_contract::Migration),
            Box::new(m20221023_002184_create_draft_pick::Migration),
            Box::new(m20221023_002185_create_draft_pick_option::Migration),
            Box::new(m20221111_002318_create_rookie_draft::Migration),
            Box::new(m20221112_132607_create_auction_tables::Migration),
            Box::new(m20221112_151717_create_trade_tables::Migration),
            Box::new(m20221117_235325_create_transaction::Migration),
            Box::new(m20230217_011454_create_team_update::Migration),
            Box::new(m20260609_000001_create_job_run::Migration),
            Box::new(m20260722_000001_collapse_transaction_fk_columns::Migration),
            Box::new(m20260725_000001_add_player_eligibility_fields::Migration),
            Box::new(m20260725_000002_alter_auction_add_status_and_owner::Migration),
            Box::new(m20260725_000003_create_auction_schedule_tables::Migration),
            Box::new(m20260730_000001_rename_auction_close_timestamps::Migration),
            Box::new(m20260731_000001_create_veteran_auction_ranking::Migration),
            Box::new(m20260731_000002_create_league_team_season_standing::Migration),
            Box::new(m20260731_000003_create_rookie_draft_lottery_tables::Migration),
            Box::new(m20260815_000001_create_rfa_resolution_tables::Migration),
            Box::new(m20260815_000002_alter_rfa_resolution_raise_deadline_nullable::Migration),
            Box::new(m20260817_000001_require_rfa_compensation_pick::Migration),
        ]
    }
}

pub async fn set_auto_updated_at_on_table(
    manager: &SchemaManager<'_>,
    table: String,
) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT set_auto_updated_at_on_table('{table}')"),
        ))
        .await?;

    Ok(())
}
