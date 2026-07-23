//! Collapses the `transaction` table's 7 mutually-exclusive nullable FK columns
//! into their real cardinalities (see beads fbkl-rust-0kb.8):
//!
//! * The 4 contract columns (`dropped`/`ir`/`rdi`/`rookie_contract_activation`)
//!   are many:1 — they collapse into one `contract_id`; the role stays
//!   recoverable from `kind`.
//! * `trade` / `auction` / `rookie_draft_selection` are 1:1 — inverted so each
//!   child owns a UNIQUE `transaction_id` FK (matching how `team_update` works).
//!
//! End state: `transaction` keeps only `contract_id` of the old FKs, set only for
//! contract-kinds. The old illegal states (>1 FK set on one row) become
//! unrepresentable, so the `before_save` validator is deleted alongside this.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{DatabaseBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

async fn run_sql(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map(|_| ())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add the new columns.
        run_sql(
            manager,
            "ALTER TABLE transaction ADD COLUMN contract_id BIGINT",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE trade ADD COLUMN transaction_id BIGINT",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction ADD COLUMN transaction_id BIGINT",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE rookie_draft_selection ADD COLUMN transaction_id BIGINT",
        )
        .await?;

        // 2. Backfill from the old columns.
        run_sql(
            manager,
            "UPDATE transaction SET contract_id = COALESCE(dropped_contract_id, ir_contract_id, rdi_contract_id, rookie_contract_activation_id)",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE trade SET transaction_id = t.id FROM transaction t WHERE t.trade_id = trade.id",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE auction SET transaction_id = t.id FROM transaction t WHERE t.auction_id = auction.id",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE rookie_draft_selection SET transaction_id = t.id FROM transaction t WHERE t.rookie_draft_selection_id = rookie_draft_selection.id",
        )
        .await?;

        // 3. Foreign keys for the new columns (cascade like the old ones did).
        run_sql(
            manager,
            "ALTER TABLE transaction ADD CONSTRAINT transaction_fk_contract FOREIGN KEY (contract_id) REFERENCES contract(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE trade ADD CONSTRAINT trade_fk_transaction FOREIGN KEY (transaction_id) REFERENCES transaction(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE auction ADD CONSTRAINT auction_fk_transaction FOREIGN KEY (transaction_id) REFERENCES transaction(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE rookie_draft_selection ADD CONSTRAINT rookie_draft_selection_fk_transaction FOREIGN KEY (transaction_id) REFERENCES transaction(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;

        // 4. Indexes: UNIQUE on the 1:1 children, plain on the many:1 contract_id.
        run_sql(
            manager,
            "CREATE UNIQUE INDEX trade_transaction ON trade(transaction_id)",
        )
        .await?;
        run_sql(
            manager,
            "CREATE UNIQUE INDEX auction_transaction ON auction(transaction_id)",
        )
        .await?;
        run_sql(
            manager,
            "CREATE UNIQUE INDEX rookie_draft_selection_transaction ON rookie_draft_selection(transaction_id)",
        )
        .await?;
        run_sql(
            manager,
            "CREATE INDEX transaction_contract ON transaction(contract_id)",
        )
        .await?;

        // 5. Drop the 7 old columns (Postgres cascades their FKs + indexes).
        run_sql(
            manager,
            "ALTER TABLE transaction \
             DROP COLUMN auction_id, \
             DROP COLUMN dropped_contract_id, \
             DROP COLUMN ir_contract_id, \
             DROP COLUMN rdi_contract_id, \
             DROP COLUMN rookie_contract_activation_id, \
             DROP COLUMN rookie_draft_selection_id, \
             DROP COLUMN trade_id",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-add the old columns.
        run_sql(
            manager,
            "ALTER TABLE transaction \
             ADD COLUMN auction_id BIGINT, \
             ADD COLUMN dropped_contract_id BIGINT, \
             ADD COLUMN ir_contract_id BIGINT, \
             ADD COLUMN rdi_contract_id BIGINT, \
             ADD COLUMN rookie_contract_activation_id BIGINT, \
             ADD COLUMN rookie_draft_selection_id BIGINT, \
             ADD COLUMN trade_id BIGINT",
        )
        .await?;

        // Backfill the contract columns by kind, and the 1:1 columns from the children.
        run_sql(
            manager,
            "UPDATE transaction SET dropped_contract_id = contract_id WHERE kind = 'TeamUpdateDropContract'",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE transaction SET ir_contract_id = contract_id WHERE kind IN ('TeamUpdateToIr', 'TeamUpdateFromIr')",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE transaction SET rdi_contract_id = contract_id WHERE kind IN ('TeamUpdateToRdi', 'TeamUpdateFromRdi')",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE transaction SET rookie_contract_activation_id = contract_id WHERE kind = 'RookieContractActivation'",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE transaction t SET trade_id = trade.id FROM trade WHERE trade.transaction_id = t.id",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE transaction t SET auction_id = auction.id FROM auction WHERE auction.transaction_id = t.id",
        )
        .await?;
        run_sql(
            manager,
            "UPDATE transaction t SET rookie_draft_selection_id = rds.id FROM rookie_draft_selection rds WHERE rds.transaction_id = t.id",
        )
        .await?;

        // Restore the old FKs.
        run_sql(
            manager,
            "ALTER TABLE transaction ADD CONSTRAINT transaction_fk_auction FOREIGN KEY (auction_id) REFERENCES auction(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE transaction ADD CONSTRAINT transaction_fk_dropped_contract FOREIGN KEY (dropped_contract_id) REFERENCES contract(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE transaction ADD CONSTRAINT transaction_fk_rookie_draft_selection FOREIGN KEY (rookie_draft_selection_id) REFERENCES rookie_draft_selection(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;
        run_sql(
            manager,
            "ALTER TABLE transaction ADD CONSTRAINT transaction_fk_trade FOREIGN KEY (trade_id) REFERENCES trade(id) ON UPDATE CASCADE ON DELETE CASCADE",
        )
        .await?;
        run_sql(
            manager,
            "CREATE INDEX transaction_dropped_contract ON transaction(dropped_contract_id)",
        )
        .await?;

        // Drop the new columns (cascades their FKs + unique indexes).
        run_sql(manager, "ALTER TABLE transaction DROP COLUMN contract_id").await?;
        run_sql(manager, "ALTER TABLE trade DROP COLUMN transaction_id").await?;
        run_sql(manager, "ALTER TABLE auction DROP COLUMN transaction_id").await?;
        run_sql(
            manager,
            "ALTER TABLE rookie_draft_selection DROP COLUMN transaction_id",
        )
        .await
    }
}
