//! A team's frozen standings inputs for one season, ingested from the external league host (§7.2).
//!
//! The rookie draft reads these rather than computing them: `mid_season_rank` is the ≈2/3-season
//! snapshot (§7.2.3) that sets lottery odds and the rounds 2–5 order, `regular_season_rank` is the
//! final standings used as a tie-break, and `playoff_finish` orders the playoff teams.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "league_team_season_standing")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub team_id: i64,
    pub end_of_season_year: i16,
    /// Final regular-season standings rank, 1 = best.
    pub regular_season_rank: i16,
    /// The ≈2/3-season snapshot rank, 1 = best.
    pub mid_season_rank: i16,
    pub made_playoffs: bool,
    /// 1 = champion … 6 = first-round loser. NULL for non-playoff teams.
    pub playoff_finish: Option<i16>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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

impl Related<super::team::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Team.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
