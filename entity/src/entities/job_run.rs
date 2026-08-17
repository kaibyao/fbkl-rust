use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A `JobRun` records one attempt (or chain of retried attempts) at processing a time-triggered league event.
///
/// Either a `deadline` row or a synthesized sub-event (auction close, RFA window
/// expiry). The unique `idempotency_key` is the double-fire guard for the scheduler.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "job_run")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub end_of_season_year: i16,
    /// Null for sub-events that are not backed by a `deadline` row (e.g. auction closes).
    pub deadline_id: Option<i64>,
    pub event_kind: JobEventKind,
    /// Which handler ran — mirrors `DeadlineKind` for deadline events, or the sub-event name.
    pub dispatch_target: String,
    pub status: JobRunStatus,
    pub attempts: i16,
    /// Stable rendering of `(league_id, end_of_season_year, kind[, auction_id])`; unique-indexed.
    pub idempotency_key: String,
    /// Links the audit `transaction` row the handler produced, if any.
    pub transaction_id: Option<i64>,
    pub error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// The category of time-triggered event a job run processed.
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Enum, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum JobEventKind {
    /// A row in the `deadline` table.
    #[sea_orm(string_value = "Deadline")]
    Deadline,
    /// An open FA auction closing after 24h with no new bids (§8.3.1).
    #[sea_orm(string_value = "FaAuctionClose")]
    FaAuctionClose,
    /// The 30-min all-bid extension chain ending (§8.3.2).
    #[sea_orm(string_value = "FaExtensionExpiry")]
    FaExtensionExpiry,
    /// A veteran auction closing after 24h with no new bids (§6.4.4).
    #[sea_orm(string_value = "VeteranAuctionClose")]
    VeteranAuctionClose,
    /// The RFA winner's 48h raise window expiring (§15.3.2).
    #[sea_orm(string_value = "RfaRaiseWindow")]
    RfaRaiseWindow,
    /// The RFA owner's 48h match window expiring (§15.3.2).
    #[sea_orm(string_value = "RfaMatchWindow")]
    RfaMatchWindow,
}

#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Enum, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum JobRunStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Running")]
    Running,
    #[sea_orm(string_value = "Succeeded")]
    Succeeded,
    #[sea_orm(string_value = "Failed")]
    Failed,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::league::Entity",
        from = "Column::LeagueId",
        to = "super::league::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    League,
    #[sea_orm(
        belongs_to = "super::deadline::Entity",
        from = "Column::DeadlineId",
        to = "super::deadline::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Deadline,
    #[sea_orm(
        belongs_to = "super::transaction::Entity",
        from = "Column::TransactionId",
        to = "super::transaction::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    Transaction,
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<super::deadline::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Deadline.def()
    }
}

impl Related<super::transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Transaction.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
