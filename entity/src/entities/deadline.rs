//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.2

use std::fmt::Debug;

use async_graphql::Enum;
use color_eyre::eyre::Result;
use fbkl_constants::league_rules::{
    KEEPER_CONTRACT_TOTAL_SALARY_LIMIT, POST_SEASON_TOTAL_SALARY_LIMIT,
    PRE_SEASON_TOTAL_SALARY_LIMIT, REGULAR_SEASON_TOTAL_SALARY_LIMIT,
};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::deadline_queries;

/// A Deadline is the date & time at which specific things happen over the course of a league season.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "deadline")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub date_time: DateTimeWithTimeZone,
    pub kind: DeadlineKind,
    pub name: String,
    pub end_of_season_year: i16,
    pub league_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl Model {
    #[instrument]
    pub async fn get_salary_cap<C>(&self, db: &C) -> Result<i16>
    where
        C: ConnectionTrait + Debug,
    {
        let salary_cap = match self.kind {
            DeadlineKind::InSeasonRosterLock => {
                let fa_auction_end_deadline = deadline_queries::find_deadline_for_season_by_type(
                    self.league_id,
                    self.end_of_season_year,
                    DeadlineKind::FreeAgentAuctionEnd,
                    db,
                )
                .await?;
                if self.date_time > fa_auction_end_deadline.date_time {
                    POST_SEASON_TOTAL_SALARY_LIMIT
                } else {
                    REGULAR_SEASON_TOTAL_SALARY_LIMIT
                }
            }
            DeadlineKind::TradeDeadlineAndPlayoffStart => POST_SEASON_TOTAL_SALARY_LIMIT,
            DeadlineKind::SeasonEnd => POST_SEASON_TOTAL_SALARY_LIMIT,
            DeadlineKind::PreseasonKeeper => KEEPER_CONTRACT_TOTAL_SALARY_LIMIT,
            DeadlineKind::PreseasonVeteranAuctionStart
            | DeadlineKind::PreseasonRookieDraftStart
            | DeadlineKind::PreseasonFaAuctionStart
            | DeadlineKind::PreseasonFaAuctionEnd
            | DeadlineKind::PreseasonFinalRosterLock => PRE_SEASON_TOTAL_SALARY_LIMIT,
            _ => REGULAR_SEASON_TOTAL_SALARY_LIMIT,
        };

        Ok(salary_cap)
    }

    #[instrument]
    pub fn is_preseason_keeper_or_before(&self) -> bool {
        [DeadlineKind::PreseasonStart, DeadlineKind::PreseasonKeeper].contains(&self.kind)
    }
}

/// The different types of deadlines that happen in a league. This is a leaky abstraction, in that there is no common way that related models use these deadline types.
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Enum, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(None)")]
pub enum DeadlineKind {
    /// The inauguration of a new season, which starts with the advancement of team contracts from the previous season.
    #[sea_orm(string_value = "PreseasonStart")]
    PreseasonStart,
    /// The first roster lock of a season that determines which contracts that have been advanced from the previous season are kept. Happens before the veteran auction and rookie draft. Cap is increased from $100 to $200 after this time.
    #[sea_orm(string_value = "PreseasonKeeper")]
    PreseasonKeeper,
    /// The start date & time of the veteran (RFA/UFA/FA) auction. Open bidding is allowed after the last predetermined contracts auction starts.
    #[sea_orm(string_value = "PreseasonVeteranAuctionStart")]
    PreseasonVeteranAuctionStart,
    /// Open bidding that starts at the time when auctions for players in the veteran auction have finished.
    #[sea_orm(string_value = "PreseasonFaAuctionStart")]
    PreseasonFaAuctionStart,
    /// The time when owners can no longer nominate bids for free agents during the preseason. Note that this is separate from the Week 1 FA auctions that are still allowed before the start of the official season.
    #[sea_orm(string_value = "PreseasonFaAuctionEnd")]
    PreseasonFaAuctionEnd,
    /// The start date & time of the rookie draft. Draft picks for the current season +2 years can be traded after this is finished.
    #[sea_orm(string_value = "PreseasonRookieDraftStart")]
    PreseasonRookieDraftStart,
    /// Following the rookie draft, rosters must be finalized by this date. Week 1 FA auctions open after this.
    #[sea_orm(string_value = "PreseasonFinalRosterLock")]
    PreseasonFinalRosterLock,
    /// The start date & time of the final free agent auction before the official start of the NBA season.
    #[sea_orm(string_value = "Week1FreeAgentAuctionStart")]
    Week1FreeAgentAuctionStart,
    /// The end date & time of the final free agent auction before the official start of the NBA season.
    #[sea_orm(string_value = "Week1FreeAgentAuctionEnd")]
    Week1FreeAgentAuctionEnd,
    /// The first weekly roster lock of the season. Coincides with the first NBA game's tipoff time.
    #[sea_orm(string_value = "Week1RosterLock")]
    Week1RosterLock,
    /// Standard weekly lock that happens on Mondays (our start of the weekly matchups) at the first tip-off of the day.
    #[sea_orm(string_value = "InSeasonRosterLock")]
    InSeasonRosterLock,
    /// Owners cannot nominate new FA auctions after this. This coincides with a $20 cap increase, which can be used for subsequent roster locks.
    #[sea_orm(string_value = "FreeAgentAuctionEnd")]
    FreeAgentAuctionEnd,
    /// Trades cannot be completed after this time. Coincides with the start of the playoffs week.
    #[sea_orm(string_value = "TradeDeadlineAndPlayoffStart")]
    TradeDeadlineAndPlayoffStart,
    /// End of the basketball season, after the last playoff week.
    #[sea_orm(string_value = "SeasonEnd")]
    SeasonEnd,
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
    #[sea_orm(has_many = "super::team_update::Entity")]
    TeamUpdate,
    #[sea_orm(has_many = "super::transaction::Entity")]
    Transaction,
}

impl Related<super::league::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::League.def()
    }
}

impl Related<super::team_update::Entity> for Entity {
    // The original relation is TeamUpdate -> Transaction -> Deadline
    // After `rev` it becomes Deadline -> Transaction -> TeamUpdate
    fn to() -> RelationDef {
        super::team_update::Relation::Transaction.def().rev()
    }

    fn via() -> Option<RelationDef> {
        Some(super::transaction::Relation::Deadline.def().rev())
    }
}

impl Related<super::transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Transaction.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
