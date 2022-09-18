/// This migration creates Postgres functions that we can call via a select statement.
/// When called, this will cause all updates to table rows that have an `updated_at` column to automatically update that column value to the current datetime.
use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DatabaseBackend, Statement},
};

/// Sets up a trigger for the given table to automatically set a column called
/// `updated_at` whenever the row is modified (unless `updated_at` was included
/// in the modified columns)
///
/// # Example
///
/// ```sql
/// CREATE TABLE users (id SERIAL PRIMARY KEY, updated_at TIMESTAMP NOT NULL DEFAULT NOW());
///
/// SELECT set_auto_updated_at_on_table('users');
/// ```
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                r#"
CREATE OR REPLACE FUNCTION set_auto_updated_at_on_table(_tbl regclass) RETURNS VOID AS $$
BEGIN
    EXECUTE format('CREATE TRIGGER set_updated_at BEFORE UPDATE ON %s
                    FOR EACH ROW EXECUTE PROCEDURE on_update_set_updated_at()', _tbl);
END;
$$ LANGUAGE plpgsql;
        "#
                .to_string(),
            ))
            .await?;

        manager
            .get_connection()
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                r#"
CREATE OR REPLACE FUNCTION on_update_set_updated_at() RETURNS trigger AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at
    ) THEN
        NEW.updated_at := current_timestamp;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
        "#
                .to_string(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                r#"
DROP FUNCTION IF EXISTS on_update_set_updated_at();"#
                    .to_string(),
            ))
            .await?;

        manager
            .get_connection()
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                r#"
DROP FUNCTION IF EXISTS set_auto_updated_at_on_table(_tbl regclass);"#
                    .to_string(),
            ))
            .await?;

        Ok(())
    }
}
