/// Re-exports `async_sea_orm_session`'s Migration so that generating a new migration file doesn't overwrite it being included in lib.rs if we were to use it directly there.
pub use async_sea_orm_session::migration::SessionTableMigration as Migration;
