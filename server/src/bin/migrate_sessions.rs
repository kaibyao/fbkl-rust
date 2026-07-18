//! One-shot: create the `tower_sessions` session table.
//!
//! The sea-orm `fbkl-migration` crate owns the app schema, but the session table
//! is created by tower-sessions' own `PostgresStore::migrate()`. The local dev
//! bin runs that on startup; serverless has no such startup, so this bin runs it
//! as part of the deploy/migration step (alongside `fbkl-migration -- up`).
//!
//! Reads `DATABASE_URL` (Supabase SESSION pooler, port 5432 — the transaction
//! pooler breaks DDL + advisory locks), the same variable the sea-orm migration
//! step uses.
//!
//!   DATABASE_URL=<session-pooler> cargo run -p fbkl-server --bin migrate_sessions

use fbkl_entity::sea_orm::Database;
use fbkl_server::build_session_store;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set (Supabase SESSION pooler)");
    let db = Database::connect(&url).await?;
    build_session_store(&db).migrate().await?;
    println!("tower_sessions schema migrated");
    Ok(())
}
