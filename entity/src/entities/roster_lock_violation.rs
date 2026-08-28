//! One roster rule broken by one team at a roster-lock deadline (rules §13.1.2, §13.2).
//!
//! An illegal roster does not block the rest of the league: lock leaves the team's `team_updates`
//! Pending and records the broken rules here, so the commissioner can read them from the API
//! instead of the scheduler's logs. Each lock run replaces the deadline's rows.

use async_graphql::Enum;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "roster_lock_violation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub deadline_id: i64,
    pub team_id: i64,
    pub rule: RosterRule,
    /// The owner-facing sentence naming what the roster broke, e.g. the count and the limit.
    pub message: String,
    pub created_at: DateTimeWithTimeZone,
}

/// Declares `RosterRule` from one list, so a rule added to the enum cannot go missing from the
/// per-rule legality flags `teamWeek` reports.
macro_rules! roster_rules {
    ($($rule:ident),+ $(,)?) => {
        /// A roster rule that a team's roster can break at lock time.
        ///
        /// Kept per-rule so callers can show a legality flag for each rule instead of one pass/fail.
        #[derive(
            Debug, Clone, Copy, Eq, PartialEq, Enum, EnumIter, DeriveActiveEnum, Serialize,
            Deserialize,
        )]
        #[sea_orm(
            rs_type = "String",
            db_type = "String(StringLen::None)",
            rename_all = "PascalCase"
        )]
        pub enum RosterRule {
            $($rule),+
        }

        impl RosterRule {
            /// Every rule, so a caller can report one legality flag per rule rather than a single pass/fail.
            pub const ALL: [Self; [$(stringify!($rule)),+].len()] = [$(Self::$rule),+];
        }
    };
}

roster_rules!(
    IrSlots,
    PreseasonRosterLimit,
    RookieDevelopmentLimit,
    IntlRookieDevelopmentLimit,
    VeteranOrRookieLimit,
    SalaryCap,
);

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
        belongs_to = "super::team::Entity",
        from = "Column::TeamId",
        to = "super::team::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Team,
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

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
