use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Debug,
};

use color_eyre::{Result, eyre::eyre};
use fbkl_entity::{
    contract, contract_queries, deadline, deadline_queries, draft_pick, draft_pick_option,
    league_event_queries,
    sea_orm::{
        ActiveModelTrait, ActiveValue, ConnectionTrait, LoaderTrait, prelude::DateTimeWithTimeZone,
    },
    team_update_queries::{TransactionStart, find_transaction_start},
    trade::{self, TradeStatus},
    trade_accommodating_drop, trade_accommodating_drop_queries,
    trade_asset::{self, TradeAssetType},
};
use tracing::instrument;

use crate::{
    drop_contract::drop_contract_from_team,
    roster::{
        RosterMoveRejection, calculate_team_contract_salary, file_and_validate_transaction,
        find_governing_deadline,
    },
};

use super::{
    create_trade_team_update::{
        generate_team_update_assets_data_for_trade, insert_team_updates_from_completed_trade,
    },
    external_trade_invalidation::invalidate_external_trades_with_traded_assets,
    process_trade_assets, validate_trade_assets,
};

static EMPTY_VEC: &Vec<contract::Model> = &vec![];

/// What to tell an owner whose league season has no roster lock left to fire. Shared so the trade
/// error and the roster resolver's message cannot drift apart.
pub const MISSING_ROSTER_LOCK_ADVICE: &str = "weekly locks run through the playoff weeks to the end of the season, so ask the commissioner to add the season's missing lock deadlines";

/// The trade's league season has no roster lock still to fire, so its adds have no week to join.
///
/// Concrete (not an opaque `eyre!`) so the resolver can `downcast_ref` and tell the owner the
/// league's lock deadlines are missing, instead of reporting a bare server fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "cannot process a trade for league (id = {league_id}) season {end_of_season_year}: no roster lock is still to fire, so the trade's adds have no week to be judged in; {MISSING_ROSTER_LOCK_ADVICE}"
)]
pub struct MissingUpcomingRosterLock {
    pub league_id: i64,
    pub end_of_season_year: i16,
}

/// Who judges the transactions a trade files (rules §13.1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeLegality {
    /// `process_trade` judges each involved team's transaction and refuses the trade when one
    /// breaks T1 or T2. What an owner-facing accept passes: the accept is the owner's last chance
    /// to submit the drops that make the trade fit, so nothing more joins the transaction later.
    JudgeNow,
    /// The caller judges the transactions itself, because it goes on adding moves to them. The
    /// historical import drives a date's accommodating drops from its own pool after the trade
    /// applies, so it - not this - decides when a replayed transaction is legal.
    CallerJudges,
}

/// Stores the trade assets + their related models for a given trade. This exists so that we aren't constantly querying the DB for the same models all the time.
#[derive(Debug)]
#[allow(clippy::struct_field_names)] // field names mirror the trade_asset domain concept, not GraphQL/DB schema
pub struct TradeAssetRelatedModelCache {
    pub trade_asset_contracts_by_trade_asset_id:
        HashMap<i64, (trade_asset::Model, contract::Model)>,
    pub trade_asset_draft_picks_by_trade_asset_id:
        HashMap<i64, (trade_asset::Model, draft_pick::Model)>,
    pub trade_asset_draft_pick_options_by_trade_asset_id:
        HashMap<i64, (trade_asset::Model, draft_pick_option::Model)>,
}

impl TradeAssetRelatedModelCache {
    #[instrument(skip(db))]
    pub async fn from_trade_assets<C>(trade_assets: Vec<trade_asset::Model>, db: &C) -> Result<Self>
    where
        C: ConnectionTrait,
    {
        let mut contract_trade_assets = vec![];
        let mut draft_pick_trade_assets = vec![];
        let mut draft_pick_option_trade_assets = vec![];

        // first group trade assets by their type
        for traded_asset in trade_assets {
            match traded_asset.asset_type {
                TradeAssetType::Contract => contract_trade_assets.push(traded_asset),
                TradeAssetType::DraftPick => draft_pick_trade_assets.push(traded_asset),
                TradeAssetType::DraftPickOption => {
                    draft_pick_option_trade_assets.push(traded_asset);
                }
            }
        }

        let traded_contracts = contract_trade_assets.load_one(contract::Entity, db).await?;
        let trade_asset_contracts_by_trade_asset_id =
            Self::map_trade_asset_models(contract_trade_assets, traded_contracts)?;

        let traded_draft_picks = draft_pick_trade_assets
            .load_one(draft_pick::Entity, db)
            .await?;
        let trade_asset_draft_picks_by_trade_asset_id =
            Self::map_trade_asset_models(draft_pick_trade_assets, traded_draft_picks)?;

        let traded_draft_pick_options = draft_pick_option_trade_assets
            .load_one(draft_pick_option::Entity, db)
            .await?;
        let trade_asset_draft_pick_options_by_trade_asset_id = Self::map_trade_asset_models(
            draft_pick_option_trade_assets,
            traded_draft_pick_options,
        )?;

        let cache = Self {
            trade_asset_contracts_by_trade_asset_id,
            trade_asset_draft_picks_by_trade_asset_id,
            trade_asset_draft_pick_options_by_trade_asset_id,
        };

        Ok(cache)
    }

    fn map_trade_asset_models<M>(
        trade_assets: Vec<trade_asset::Model>,
        related_models: Vec<Option<M>>,
    ) -> Result<HashMap<i64, (trade_asset::Model, M)>> {
        let mut mapped_models = HashMap::new();
        for (trade_asset, maybe_related_model) in trade_assets.into_iter().zip(related_models) {
            let related_model = maybe_related_model.ok_or_else(|| {
                eyre!(
                    "Missing related model for trade asset (id = {}).",
                    trade_asset.id
                )
            })?;
            mapped_models.insert(trade_asset.id, (trade_asset, related_model));
        }

        Ok(mapped_models)
    }
}

/// Moves a created trade's assets between teams and completes it.
///
/// Updates the trade status to `completed`, creates the league event, and invalidates every other
/// pending trade that includes any of the traded assets. Returns the updated trade model.
///
/// A trade is a transaction, and a transaction is judged at the lock it is filed under (rules
/// §13.1.4-§13.1.6), so the trade files under the lock still to fire - not the next deadline of
/// any kind, which can sit before that lock and put the trade in a week it was never judged in.
///
/// The trade's `team_update` snapshots report the cap in force when the trade was made, not the
/// coming lock's, which is the same deadline its transactions are judged against.
#[instrument(skip(db))]
pub async fn process_trade<C>(
    trade_model: trade::Model,
    trade_datetime: &DateTimeWithTimeZone,
    legality: TradeLegality,
    db: &C,
) -> Result<trade::Model>
where
    C: ConnectionTrait,
{
    let upcoming_lock = deadline_queries::find_upcoming_roster_lock(
        trade_model.league_id,
        trade_model.end_of_season_year,
        *trade_datetime,
        db,
    )
    .await?
    .ok_or(MissingUpcomingRosterLock {
        league_id: trade_model.league_id,
        end_of_season_year: trade_model.end_of_season_year,
    })?;
    let salary_snapshot_deadline =
        find_governing_deadline(trade_datetime, &upcoming_lock, db).await?;
    let traded_trade_assets = trade_model.get_trade_assets(db).await?;
    let mut all_team_ids = HashSet::new();
    for traded_trade_asset in &traded_trade_assets {
        all_team_ids.insert(traded_trade_asset.from_team_id);
        all_team_ids.insert(traded_trade_asset.to_team_id);
    }
    let trade_asset_related_models =
        TradeAssetRelatedModelCache::from_trade_assets(traded_trade_assets, db).await?;
    validate_trade_assets(&trade_asset_related_models, trade_model.id, db).await?;

    let active_contracts_by_team_id = contract_queries::find_active_contracts_by_teams(
        all_team_ids.iter().copied().collect(),
        db,
    )
    .await?;
    let mut team_salaries_before_trade = HashMap::new();
    for team_id in &all_team_ids {
        let team_active_contracts = active_contracts_by_team_id
            .get_vec(team_id)
            .unwrap_or(EMPTY_VEC);
        let team_salary_and_cap = calculate_team_contract_salary(
            *team_id,
            team_active_contracts,
            &salary_snapshot_deadline,
            db,
        )
        .await?;
        team_salaries_before_trade.insert(*team_id, team_salary_and_cap);
    }

    let trade_transactions =
        TradeTransactions::read(trade_model.id, &all_team_ids, &upcoming_lock, db).await?;

    // process trade / create new contracts
    let updated_trade_asset_models = process_trade_assets(&trade_asset_related_models, db).await?;
    let updated_trade = update_trade_status(trade_model, db).await?;

    // create league event
    let trade_league_event =
        league_event_queries::insert_trade_league_event(&upcoming_lock, updated_trade.id, db)
            .await?;

    // Create team_update
    let trade_asset_contracts: Vec<(trade_asset::Model, contract::Model)> =
        trade_asset_related_models
            .trade_asset_contracts_by_trade_asset_id
            .values()
            .map(|(trade_asset_model, model)| (trade_asset_model.clone(), model.clone()))
            .collect();
    let trade_asset_draft_picks: Vec<(trade_asset::Model, draft_pick::Model)> =
        trade_asset_related_models
            .trade_asset_draft_picks_by_trade_asset_id
            .values()
            .map(|(trade_asset_model, model)| (trade_asset_model.clone(), model.clone()))
            .collect();
    let trade_asset_draft_pick_options: Vec<(trade_asset::Model, draft_pick_option::Model)> =
        trade_asset_related_models
            .trade_asset_draft_pick_options_by_trade_asset_id
            .values()
            .map(|(trade_asset_model, model)| (trade_asset_model.clone(), model.clone()))
            .collect();
    let team_update_assets_by_team_id = generate_team_update_assets_data_for_trade(
        &trade_asset_contracts,
        &trade_asset_draft_picks,
        &trade_asset_draft_pick_options,
        &updated_trade_asset_models.contracts_by_trade_asset_id,
        db,
    )
    .await?;
    insert_team_updates_from_completed_trade(
        team_update_assets_by_team_id,
        trade_datetime,
        &trade_league_event,
        &salary_snapshot_deadline,
        &team_salaries_before_trade,
        all_team_ids.into_iter().collect(),
        db,
    )
    .await?;

    trade_transactions
        .apply_drops_and_validate(
            &trade_asset_contracts,
            &updated_trade_asset_models.contracts_by_trade_asset_id,
            &upcoming_lock,
            trade_datetime,
            legality,
            db,
        )
        .await?;

    invalidate_external_trades_with_traded_assets(&updated_trade, &trade_asset_related_models, db)
        .await?;

    Ok(updated_trade)
}

/// The transactions a trade files: one per involved team, each holding that team's legs and the
/// drops that owner submitted to accommodate them (rules §12.5.3, §13.1.4).
#[derive(Debug)]
struct TradeTransactions {
    accommodating_drops: Vec<trade_accommodating_drop::Model>,
    /// Where each team's transaction starts, read before the trade writes anything. Keyed in team
    /// id order so a refused trade names the same team every run, and so two trades that share two
    /// teams cannot deadlock on the team rows `find_transaction_start` locks.
    starts_by_team_id: BTreeMap<i64, TransactionStart>,
}

impl TradeTransactions {
    /// Reads the drops and each team's watermark. Call before the trade's first write: the
    /// watermark marks off the `team_update` rows this trade is about to write, and the drops are
    /// one transaction with the legs.
    #[instrument(skip(db))]
    async fn read<C>(
        trade_id: i64,
        asset_team_ids: &HashSet<i64>,
        upcoming_lock: &deadline::Model,
        db: &C,
    ) -> Result<Self>
    where
        C: ConnectionTrait,
    {
        let accommodating_drops =
            trade_accommodating_drop_queries::find_accommodating_drops_for_trade(trade_id, db)
                .await?;

        let mut team_ids: BTreeSet<i64> = asset_team_ids.iter().copied().collect();
        team_ids.extend(
            accommodating_drops
                .iter()
                .map(|accommodating_drop| accommodating_drop.team_id),
        );

        let mut starts_by_team_id = BTreeMap::new();
        for team_id in team_ids {
            starts_by_team_id.insert(
                team_id,
                find_transaction_start(team_id, upcoming_lock.id, db).await?,
            );
        }

        Ok(Self {
            accommodating_drops,
            starts_by_team_id,
        })
    }

    /// Applies each owner's drops, then judges every involved team's transaction (T1 and T2).
    ///
    /// Every team's T1 failures are gathered, teams in id order, so an owner fixing a refused trade
    /// reads all of them at once instead of one per retry.
    ///
    /// A drop may name a contract the trade brings in, whose row `process_trade_assets` has already
    /// replaced; the trade asset's replacement is the row to remove, which puts the add and the
    /// removal in one transaction and is what T2 refuses. A drop naming a contract the team no
    /// longer holds - one it traded away in this same trade - has nothing to remove and is refused.
    #[instrument(skip(db))]
    async fn apply_drops_and_validate<C>(
        &self,
        trade_asset_contracts: &[(trade_asset::Model, contract::Model)],
        replacement_contracts_by_trade_asset_id: &HashMap<i64, contract::Model>,
        upcoming_lock: &deadline::Model,
        trade_datetime: &DateTimeWithTimeZone,
        legality: TradeLegality,
        db: &C,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let mut replacements_by_traded_contract_id = HashMap::new();
        for (trade_asset_model, traded_contract) in trade_asset_contracts {
            if let Some(replacement) =
                replacement_contracts_by_trade_asset_id.get(&trade_asset_model.id)
            {
                replacements_by_traded_contract_id.insert(traded_contract.id, replacement.clone());
            }
        }

        for accommodating_drop in &self.accommodating_drops {
            let contract_model =
                match replacements_by_traded_contract_id.get(&accommodating_drop.contract_id) {
                    Some(replacement) => replacement.clone(),
                    None => {
                        contract_queries::find_contract_by_id(accommodating_drop.contract_id, db)
                            .await?
                    }
                };
            if contract_model.team_id != Some(accommodating_drop.team_id) {
                return Err(RosterMoveRejection::AccommodatingDropNotOnRoster {
                    contract_id: accommodating_drop.contract_id,
                    team_id: accommodating_drop.team_id,
                }
                .into());
            }

            drop_contract_from_team(contract_model, upcoming_lock, db).await?;
        }

        if legality == TradeLegality::CallerJudges {
            return Ok(());
        }

        let mut violations = vec![];
        for (team_id, transaction_start) in &self.starts_by_team_id {
            let Err(report) = file_and_validate_transaction(
                *team_id,
                upcoming_lock,
                transaction_start,
                trade_datetime,
                db,
            )
            .await
            else {
                continue;
            };
            // Only T1 is gathered: every other rejection names one move, so it needs no other team.
            match report.downcast_ref::<RosterMoveRejection>() {
                Some(RosterMoveRejection::TransactionLeavesRosterIllegal {
                    violations: team_violations,
                    ..
                }) => violations.extend(team_violations.iter().cloned()),
                _ => return Err(report),
            }
        }
        if !violations.is_empty() {
            return Err(RosterMoveRejection::TradeLeavesRostersIllegal { violations }.into());
        }

        Ok(())
    }
}

#[instrument(skip(db))]
async fn update_trade_status<C>(trade_model: trade::Model, db: &C) -> Result<trade::Model>
where
    C: ConnectionTrait,
{
    let mut trade_to_update: trade::ActiveModel = trade_model.into();
    trade_to_update.status = ActiveValue::Set(TradeStatus::Completed);
    let updated_trade = trade_to_update.update(db).await?;

    Ok(updated_trade)
}
