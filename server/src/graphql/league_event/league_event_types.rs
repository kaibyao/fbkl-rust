use async_graphql::{InputObject, SimpleObject};
use fbkl_entity::league_event::{self, LeagueEventKind};

/// One recorded league state change. Covers every `LeagueEventKind` variant; the kind tells the
/// client which child row (trade, auction, rookie draft selection, team update) carries the detail.
#[derive(SimpleObject)]
pub struct LeagueEvent {
    pub id: i64,
    pub end_of_season_year: i16,
    pub kind: LeagueEventKind,
    pub league_id: i64,
    pub deadline_id: i64,
    pub contract_id: Option<i64>,
    pub created_at: String,
}

impl LeagueEvent {
    pub(super) fn from_model(entity: &league_event::Model) -> Self {
        Self {
            id: entity.id,
            end_of_season_year: entity.end_of_season_year,
            kind: entity.kind,
            league_id: entity.league_id,
            deadline_id: entity.deadline_id,
            contract_id: entity.contract_id,
            created_at: entity.created_at.to_string(),
        }
    }
}

/// One page of the audit feed. `totalItems` is the count for the unpaginated filter, so the client
/// can render page controls.
#[derive(SimpleObject)]
pub struct PagedLeagueEvents {
    pub items: Vec<LeagueEvent>,
    pub total_items: u64,
}

/// Narrows the feed. Both fields are optional; omitting them returns the whole league's history.
#[derive(InputObject)]
pub struct LeagueEventFilter {
    pub team_id: Option<i64>,
    pub kind: Option<LeagueEventKind>,
}
