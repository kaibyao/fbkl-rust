use std::{collections::HashMap, fmt::Debug};

use color_eyre::eyre::{Result, eyre};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, JoinType,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use tracing::instrument;

use crate::{
    contract,
    deadline::{self, DeadlineKind},
    deadline_queries, draft_pick,
    rookie_draft_selection::{self, RookieDraftSelectionStatus},
    transaction::{self, TransactionKind},
};

/// Rookie draft selections made in a league's season, in draft order. The season comes from the
/// selection's draft pick.
#[instrument]
pub async fn find_rookie_draft_selections_for_league_season<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<rookie_draft_selection::Model>>
where
    C: ConnectionTrait + Debug,
{
    let selections = rookie_draft_selection::Entity::find()
        .join(
            JoinType::InnerJoin,
            rookie_draft_selection::Relation::DraftPick.def(),
        )
        .filter(rookie_draft_selection::Column::LeagueId.eq(league_id))
        .filter(draft_pick::Column::EndOfSeasonYear.eq(end_of_season_year))
        .order_by_asc(rookie_draft_selection::Column::Order)
        .all(db)
        .await?;

    Ok(selections)
}

/// The full ordered slate for the season's draft — the board.
pub use find_rookie_draft_selections_for_league_season as get_selections_for_draft;

/// The selection that is on the clock: the lowest-`order` `Unused` row. `None` once every pick is
/// used or skipped.
#[instrument]
pub async fn get_on_the_clock_selection<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Option<rookie_draft_selection::Model>>
where
    C: ConnectionTrait + Debug,
{
    let selection = rookie_draft_selection::Entity::find()
        .join(
            JoinType::InnerJoin,
            rookie_draft_selection::Relation::DraftPick.def(),
        )
        .filter(rookie_draft_selection::Column::LeagueId.eq(league_id))
        .filter(draft_pick::Column::EndOfSeasonYear.eq(end_of_season_year))
        .filter(rookie_draft_selection::Column::Status.eq(RookieDraftSelectionStatus::Unused))
        .order_by_asc(rookie_draft_selection::Column::Order)
        .one(db)
        .await?;

    Ok(selection)
}

/// Pre-creates the season's whole ordered slate as `Unused` rows, so the UI can show who is on the
/// clock before any pick is made (the importer instead back-fills `order` per imported pick).
///
/// No-op if the slate already exists, which is what makes `start_rookie_draft` safe to call twice.
#[instrument(skip(ordered_draft_pick_ids))]
pub async fn build_draft_slate<C>(
    league_id: i64,
    end_of_season_year: i16,
    ordered_draft_pick_ids: Vec<i64>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    if ordered_draft_pick_ids.is_empty()
        || !get_selections_for_draft(league_id, end_of_season_year, db)
            .await?
            .is_empty()
    {
        return Ok(());
    }

    let owner_team_id_by_draft_pick_id: HashMap<i64, i64> = draft_pick::Entity::find()
        .filter(draft_pick::Column::Id.is_in(ordered_draft_pick_ids.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|draft_pick_model| (draft_pick_model.id, draft_pick_model.current_owner_team_id))
        .collect();

    let mut models_to_insert = Vec::with_capacity(ordered_draft_pick_ids.len());
    for (index, draft_pick_id) in ordered_draft_pick_ids.into_iter().enumerate() {
        let current_owner_team_id = *owner_team_id_by_draft_pick_id
            .get(&draft_pick_id)
            .ok_or_else(|| eyre!("Could not find draft pick ({draft_pick_id}) for draft slate."))?;
        let order = i16::try_from(index + 1)?;
        models_to_insert.push(rookie_draft_selection::ActiveModel {
            order: ActiveValue::Set(order),
            status: ActiveValue::Set(RookieDraftSelectionStatus::Unused),
            draft_pick_id: ActiveValue::Set(draft_pick_id),
            league_id: ActiveValue::Set(league_id),
            current_owner_team_id: ActiveValue::Set(current_owner_team_id),
            ..Default::default()
        });
    }

    rookie_draft_selection::Entity::insert_many(models_to_insert)
        .exec(db)
        .await?;

    Ok(())
}

/// Contracts dropped while the draft was running, for the §7.3.4 re-draft ban.
///
/// The draft window is the `PreseasonRookieDraftStart` deadline through the following
/// `PreseasonFinalRosterLock`, both inclusive — §7.3.3 allows drops in that window, §7.3.4 bans
/// re-drafting those players.
#[instrument]
pub async fn find_players_dropped_during_draft<C>(
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<contract::Model>>
where
    C: ConnectionTrait + Debug,
{
    let draft_start_deadline = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::PreseasonRookieDraftStart,
        db,
    )
    .await?;
    let roster_lock_deadline = deadline_queries::find_deadline_for_season_by_type(
        league_id,
        end_of_season_year,
        DeadlineKind::PreseasonFinalRosterLock,
        db,
    )
    .await?;

    let dropped_contracts = contract::Entity::find()
        .join(
            JoinType::InnerJoin,
            contract::Relation::DroppedContractTransaction.def(),
        )
        .join(JoinType::InnerJoin, transaction::Relation::Deadline.def())
        .filter(contract::Column::LeagueId.eq(league_id))
        .filter(transaction::Column::Kind.eq(TransactionKind::TeamUpdateDropContract))
        .filter(deadline::Column::DateTime.between(
            draft_start_deadline.date_time,
            roster_lock_deadline.date_time,
        ))
        .all(db)
        .await?;

    Ok(dropped_contracts)
}

#[instrument]
pub async fn insert_used_rookie_draft_selection<C>(
    signed_rookie_contract: &contract::Model,
    draft_pick_id: i64,
    overall_draft_rank: i16,
    db: &C,
) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + Debug,
{
    let rookie_draft_selection_to_insert = rookie_draft_selection::Model::from_rookie_contract(
        signed_rookie_contract.league_id,
        signed_rookie_contract.id,
        draft_pick_id,
        current_owner_team_id_for_draft_pick(draft_pick_id, db).await?,
        overall_draft_rank,
    );
    let inserted_rookie_draft_selection = rookie_draft_selection_to_insert.insert(db).await?;
    Ok(inserted_rookie_draft_selection)
}

#[instrument]
pub async fn insert_skipped_rookie_draft_selection<C>(
    league_id: i64,
    draft_pick_id: i64,
    overall_draft_rank: i16,
    db: &C,
) -> Result<rookie_draft_selection::Model>
where
    C: ConnectionTrait + Debug,
{
    let rookie_draft_selection_to_insert = rookie_draft_selection::ActiveModel {
        order: ActiveValue::Set(overall_draft_rank),
        status: ActiveValue::Set(RookieDraftSelectionStatus::Skipped),
        contract_id: ActiveValue::NotSet,
        draft_pick_id: ActiveValue::Set(draft_pick_id),
        league_id: ActiveValue::Set(league_id),
        current_owner_team_id: ActiveValue::Set(
            current_owner_team_id_for_draft_pick(draft_pick_id, db).await?,
        ),
        ..Default::default()
    };
    let inserted_rookie_draft_selection = rookie_draft_selection_to_insert.insert(db).await?;
    Ok(inserted_rookie_draft_selection)
}

/// Keeps the lazily-inserted (importer) paths filling the denormalized owner column without
/// needing the caller to hand over a draft pick model.
#[instrument]
async fn current_owner_team_id_for_draft_pick<C>(draft_pick_id: i64, db: &C) -> Result<i64>
where
    C: ConnectionTrait + Debug,
{
    let draft_pick_model = draft_pick::Entity::find_by_id(draft_pick_id)
        .one(db)
        .await?
        .ok_or_else(|| eyre!("Could not find draft pick ({draft_pick_id})."))?;
    Ok(draft_pick_model.current_owner_team_id)
}
