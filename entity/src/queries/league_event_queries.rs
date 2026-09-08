use color_eyre::{Result, eyre::eyre};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, JoinType, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, sea_query::Expr,
};
use tracing::instrument;

use crate::{
    auction,
    deadline::{self, DeadlineKind},
    league_event::{self, LeagueEventKind},
    queries::pagination::{Paged, fetch_page},
    rookie_draft_selection, team_update, trade,
};

#[instrument(skip(db))]
pub async fn find_league_event_by_id<C>(league_event_id: i64, db: &C) -> Result<league_event::Model>
where
    C: ConnectionTrait,
{
    league_event::Entity::find_by_id(league_event_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find league event with id: {league_event_id}"))
}

/// One page of a league's league event audit feed, newest first, optionally narrowed to a single
/// team or `LeagueEventKind`. The feed spans the league's whole history, so it is never unbounded.
///
/// The team filter joins `team_update` (a league event carries no `team_id` of its own), which is
/// why the select is `DISTINCT` — an event touching both sides of a trade has two updates.
#[instrument(skip(db))]
pub async fn find_league_events_in_league<C>(
    league_id: i64,
    maybe_team_id: Option<i64>,
    maybe_kind: Option<LeagueEventKind>,
    page: u64,
    page_size: u64,
    db: &C,
) -> Result<Paged<league_event::Model>>
where
    C: ConnectionTrait,
{
    let mut query = league_event::Entity::find()
        .filter(league_event::Column::LeagueId.eq(league_id))
        .order_by_desc(league_event::Column::Id);

    if let Some(kind) = maybe_kind {
        query = query.filter(league_event::Column::Kind.eq(kind));
    }

    if let Some(team_id) = maybe_team_id {
        query = query
            .join(
                JoinType::InnerJoin,
                league_event::Relation::TeamUpdate.def(),
            )
            .filter(team_update::Column::TeamId.eq(team_id))
            .distinct();
    }

    fetch_page(query, page, page_size, db).await
}

/// The league's keeper league event for a season, if keepers have been touched at all yet.
#[instrument(skip(db))]
pub async fn find_keeper_deadline_league_event<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Option<league_event::Model>>
where
    C: ConnectionTrait,
{
    let found = league_event::Entity::find()
        .filter(
            league_event::Column::Kind
                .eq(LeagueEventKind::PreseasonKeeper)
                .and(league_event::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(league_event::Column::LeagueId.eq(league_id)),
        )
        .one(db)
        .await?;

    Ok(found)
}

#[instrument(skip(db))]
pub async fn get_or_create_keeper_deadline_league_event<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<league_event::Model>
where
    C: ConnectionTrait,
{
    let maybe_existing_keeper_deadline_league_event =
        find_keeper_deadline_league_event(league_id, end_of_season_year, db).await?;

    if let Some(existing_keeper_deadline_league_event) = maybe_existing_keeper_deadline_league_event
    {
        return Ok(existing_keeper_deadline_league_event);
    }

    let maybe_keeper_deadline = deadline::Entity::find()
        .filter(
            deadline::Column::LeagueId
                .eq(league_id)
                .and(deadline::Column::EndOfSeasonYear.eq(end_of_season_year))
                .and(deadline::Column::Kind.eq(DeadlineKind::PreseasonKeeper)),
        )
        .one(db)
        .await?;
    let keeper_deadline = maybe_keeper_deadline.ok_or_else(|| eyre!("Keeper deadline for league ({}) & season end year ({}) not found! Have deadlines for this league been generated?", league_id, end_of_season_year))?;

    let league_event_to_insert =
        league_event::Model::new_keeper_deadline_league_event(&keeper_deadline);
    insert_league_event(league_event_to_insert, db).await
}

/// Creates & inserts a league event tied to the end of an auction, then points the auction's 1:1 `league_event_id` FK back at it.
#[instrument(skip(db))]
pub async fn insert_auction_league_event<C>(
    deadline_model: &deadline::Model,
    auction_id: i64,
    db: &C,
) -> Result<league_event::Model>
where
    C: ConnectionTrait,
{
    let league_event_model = insert_league_event(
        league_event::Model::new_auction_league_event(deadline_model),
        db,
    )
    .await?;
    auction::Entity::update_many()
        .col_expr(
            auction::Column::LeagueEventId,
            Expr::value(league_event_model.id),
        )
        .filter(auction::Column::Id.eq(auction_id))
        .exec(db)
        .await?;
    Ok(league_event_model)
}

/// Creates & inserts a league event tied to a completed trade, then points the trade's 1:1 `league_event_id` FK back at it.
#[instrument(skip(db))]
pub async fn insert_trade_league_event<C>(
    deadline_model: &deadline::Model,
    trade_id: i64,
    db: &C,
) -> Result<league_event::Model>
where
    C: ConnectionTrait,
{
    let league_event_model = insert_league_event(
        league_event::Model::new_trade_league_event(deadline_model),
        db,
    )
    .await?;
    trade::Entity::update_many()
        .col_expr(
            trade::Column::LeagueEventId,
            Expr::value(league_event_model.id),
        )
        .filter(trade::Column::Id.eq(trade_id))
        .exec(db)
        .await?;
    Ok(league_event_model)
}

/// Creates & inserts a league event tied to a rookie draft selection, then points the selection's 1:1 `league_event_id` FK back at it.
#[instrument(skip(db))]
pub async fn insert_rookie_draft_selection_league_event<C>(
    deadline_model: &deadline::Model,
    rookie_draft_selection_id: i64,
    db: &C,
) -> Result<league_event::Model>
where
    C: ConnectionTrait,
{
    let league_event_model = insert_league_event(
        league_event::Model::new_rookie_draft_selection_league_event(deadline_model),
        db,
    )
    .await?;
    rookie_draft_selection::Entity::update_many()
        .col_expr(
            rookie_draft_selection::Column::LeagueEventId,
            Expr::value(league_event_model.id),
        )
        .filter(rookie_draft_selection::Column::Id.eq(rookie_draft_selection_id))
        .exec(db)
        .await?;
    Ok(league_event_model)
}

#[instrument(skip(db))]
pub async fn insert_league_event<C>(
    league_event_to_insert: league_event::ActiveModel,
    db: &C,
) -> Result<league_event::Model>
where
    C: ConnectionTrait,
{
    let inserted_league_event = league_event_to_insert.insert(db).await?;
    Ok(inserted_league_event)
}
