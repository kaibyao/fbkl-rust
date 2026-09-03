//! Renames the `transaction` table to `league_event` (beads fbkl-rust-d1r.17).
//!
//! The league's own word for a team's atomic weekly unit is "transaction" (commissioner ruling,
//! 2021-11-01). This table holds something else: one row per recorded league state change - a
//! trade, an auction close, a drop, a keeper deadline, an RFA decision. Several of its kinds
//! (`PreseasonStart`, `RfaResign`, ...) belong to no team at all, and one `Trade` row is shared by
//! every team in the trade, so the row is an event, not a team's transaction. The weekly unit is
//! `team_update.sequence` instead.
//!
//! Pure rename: table, its identity sequence, its indexes and constraints, and the
//! `transaction_id` FK column on the five referencing tables. `kind` string values are stored data
//! and stay as they are.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{DatabaseBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

async fn run_sql(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map(|_| ())
}

/// Tables holding a FK to the renamed table, each with the index name on that FK column (if any).
const REFERENCING_TABLES: [(&str, Option<&str>); 5] = [
    ("team_update", Some("team_update_transaction")),
    ("trade", Some("trade_transaction")),
    ("auction", Some("auction_transaction")),
    (
        "rookie_draft_selection",
        Some("rookie_draft_selection_transaction"),
    ),
    ("job_run", None),
];

const OWN_INDEXES: [(&str, &str); 5] = [
    ("transaction_pkey", "league_event_pkey"),
    ("transaction_contract", "league_event_contract"),
    ("transaction_deadline", "league_event_deadline"),
    ("transaction_kind", "league_event_kind"),
    ("transaction_league_year", "league_event_league_year"),
];

const OWN_CONSTRAINTS: [&str; 3] = ["contract", "deadline", "league"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        run_sql(manager, "ALTER TABLE transaction RENAME TO league_event").await?;
        run_sql(
            manager,
            "ALTER SEQUENCE IF EXISTS transaction_id_seq RENAME TO league_event_id_seq",
        )
        .await?;

        for (old, new) in OWN_INDEXES {
            run_sql(manager, &format!("ALTER INDEX {old} RENAME TO {new}")).await?;
        }
        for target in OWN_CONSTRAINTS {
            run_sql(
                manager,
                &format!(
                    "ALTER TABLE league_event RENAME CONSTRAINT transaction_fk_{target} TO league_event_fk_{target}"
                ),
            )
            .await?;
        }

        for (table, index) in REFERENCING_TABLES {
            run_sql(
                manager,
                &format!("ALTER TABLE {table} RENAME COLUMN transaction_id TO league_event_id"),
            )
            .await?;
            run_sql(
                manager,
                &format!(
                    "ALTER TABLE {table} RENAME CONSTRAINT {table}_fk_transaction TO {table}_fk_league_event"
                ),
            )
            .await?;
            if let Some(index) = index {
                run_sql(
                    manager,
                    &format!("ALTER INDEX {index} RENAME TO {table}_league_event"),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (table, index) in REFERENCING_TABLES {
            if let Some(index) = index {
                run_sql(
                    manager,
                    &format!("ALTER INDEX {table}_league_event RENAME TO {index}"),
                )
                .await?;
            }
            run_sql(
                manager,
                &format!(
                    "ALTER TABLE {table} RENAME CONSTRAINT {table}_fk_league_event TO {table}_fk_transaction"
                ),
            )
            .await?;
            run_sql(
                manager,
                &format!("ALTER TABLE {table} RENAME COLUMN league_event_id TO transaction_id"),
            )
            .await?;
        }

        for target in OWN_CONSTRAINTS {
            run_sql(
                manager,
                &format!(
                    "ALTER TABLE league_event RENAME CONSTRAINT league_event_fk_{target} TO transaction_fk_{target}"
                ),
            )
            .await?;
        }
        for (old, new) in OWN_INDEXES {
            run_sql(manager, &format!("ALTER INDEX {new} RENAME TO {old}")).await?;
        }

        run_sql(
            manager,
            "ALTER SEQUENCE IF EXISTS league_event_id_seq RENAME TO transaction_id_seq",
        )
        .await?;
        run_sql(manager, "ALTER TABLE league_event RENAME TO transaction").await
    }
}
