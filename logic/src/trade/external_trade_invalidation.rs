use std::fmt::Debug;

use color_eyre::{eyre::eyre, Result};
use fbkl_entity::{
    contract, draft_pick,
    draft_pick_option::{self, DraftPickOptionStatus},
    draft_pick_queries,
    sea_orm::{
        sea_query::Expr, ColumnTrait, ConnectionTrait, EntityTrait, LoaderTrait, QueryFilter,
    },
    trade::{self, TradeStatus},
    trade_asset::{self, TradeAssetType},
};
use tracing::instrument;

use super::process_trade::TradeAssetRelatedModelCache;

/// Invalidates other trades involving assets that were just traded.
#[instrument]
pub async fn invalidate_external_trades_with_traded_assets<C>(
    completed_trade: &trade::Model,
    trade_asset_related_models: &TradeAssetRelatedModelCache,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    /*
    Relational paths for getting to the external trades affected by assets traded:
    * trade asset -> contract|draft_pick -> trade assets -> trades
    * trade asset -> draft_pick_option -> draft_picks -> trade assets -> trade
    * trade asset -> draft_pick_option_amendment -> draft_pick_option -> draft_picks -> trade assets -> trade

    Since this is potentially many database calls and we want to avoid N + 1 queries, we should try to make sure these related models can be fetched in as few calls as possible.

    Based on the above paths, the point in which all paths merge is at: trade_assets (many) -> trades (many). We should use a Loader pattern for this query.

    Before that, draft_pick_option merges on draft_pick. The final merge paths look like the following:

    trade_asset --> draft_pick_option --\
    trade_asset ----------------------> draft_pick -\
    trade_asset ----------------------> contract ---\
                                                    trade_asset ----\
                                                                    trade
    */

    let mut all_external_affected_trade_assets: Vec<trade_asset::Model> = vec![];

    // merge contract to trade_asset
    let traded_contracts: Vec<contract::Model> = trade_asset_related_models
        .trade_asset_contracts_by_trade_asset_id
        .values()
        .map(|(_trade_asset, contract_model)| contract_model.clone())
        .collect();
    let external_trade_assets_with_traded_contacts =
        get_external_trade_assets_related_to_traded_contracts(&traded_contracts, db).await?;
    all_external_affected_trade_assets.extend(external_trade_assets_with_traded_contacts);

    // merge draft_pick_option to draft_pick
    let traded_draft_pick_options: Vec<draft_pick_option::Model> = trade_asset_related_models
        .trade_asset_draft_pick_options_by_trade_asset_id
        .values()
        .map(|(_trade_asset, draft_pick_option_model)| draft_pick_option_model.clone())
        .collect();
    let draft_picks_affected_by_affected_options =
        draft_pick_queries::get_draft_picks_affected_by_options(&traded_draft_pick_options, db)
            .await?;

    // merge draft_pick to trade_asset
    let traded_draft_picks: Vec<draft_pick::Model> = trade_asset_related_models
        .trade_asset_draft_picks_by_trade_asset_id
        .values()
        .map(|(_trade_asset, draft_pick_model)| draft_pick_model.clone())
        .collect();
    let external_trade_asset_with_affected_draft_picks =
        get_external_trade_assets_related_to_affected_draft_picks(
            traded_draft_picks,
            draft_picks_affected_by_affected_options,
            db,
        )
        .await?;
    all_external_affected_trade_assets.extend(external_trade_asset_with_affected_draft_picks);

    // get external trades
    all_external_affected_trade_assets
        .retain(|trade_asset| trade_asset.trade_id != completed_trade.id);
    let mut all_active_external_trades_affected_by_traded_assets: Vec<trade::Model> =
        all_external_affected_trade_assets
            .load_many(trade::Entity, db)
            .await?
            .into_iter()
            .flatten()
            .filter(|trade| {
                trade.is_active()
                    && trade.league_id == completed_trade.league_id
                    && trade.end_of_season_year == completed_trade.end_of_season_year
            })
            .collect();
    all_active_external_trades_affected_by_traded_assets.dedup_by_key(|trade| trade.id);

    invalidate_external_trades(all_active_external_trades_affected_by_traded_assets, db).await
}

#[instrument]
async fn get_external_trade_assets_related_to_traded_contracts<C>(
    traded_contracts: &[contract::Model],
    db: &C,
) -> Result<impl Iterator<Item = trade_asset::Model>>
where
    C: ConnectionTrait + Debug,
{
    let external_trade_assets_with_traded_contacts = traded_contracts
        .load_many(trade_asset::Entity, db)
        .await?
        .into_iter()
        .flatten();

    Ok(external_trade_assets_with_traded_contacts)
}

#[instrument]
async fn get_external_trade_assets_related_to_affected_draft_picks<C>(
    traded_draft_picks: Vec<draft_pick::Model>,
    draft_picks_affected_by_affected_options: Vec<draft_pick::Model>,
    db: &C,
) -> Result<impl Iterator<Item = trade_asset::Model>>
where
    C: ConnectionTrait + Debug,
{
    let mut all_affected_draft_picks =
        vec![draft_picks_affected_by_affected_options, traded_draft_picks].concat();
    all_affected_draft_picks.dedup_by_key(|draft_pick| draft_pick.id);
    let external_trade_asset_with_affected_draft_picks = all_affected_draft_picks
        .load_many(trade_asset::Entity, db)
        .await?
        .into_iter()
        .flatten();

    Ok(external_trade_asset_with_affected_draft_picks)
}

#[instrument]
async fn invalidate_external_trades<C>(external_trades: Vec<trade::Model>, db: &C) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let external_trade_ids = external_trades.iter().map(|trade| trade.id);

    trade::Entity::update_many()
        .col_expr(
            trade::Column::Status,
            Expr::value(TradeStatus::InvalidatedByExternalTrade),
        )
        .filter(trade::Column::Id.is_in(external_trade_ids))
        .exec(db)
        .await?;

    invalidate_external_trade_assets(external_trades, db).await?;

    Ok(())
}

#[instrument]
async fn invalidate_external_trade_assets<C>(
    external_trades: Vec<trade::Model>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let external_trade_assets: Vec<trade_asset::Model> = external_trades
        .load_many(trade_asset::Entity, db)
        .await?
        .into_iter()
        .flatten()
        .collect();

    let mut external_draft_pick_option_trade_assets = vec![];

    // first group trade assets by their type
    for external_trade_asset in external_trade_assets {
        // don't care about contracts and draft picks, as they've already been updated
        if TradeAssetType::DraftPickOption == external_trade_asset.asset_type {
            external_draft_pick_option_trade_assets.push(external_trade_asset)
        };
    }

    invalidate_external_trade_draft_pick_options(external_draft_pick_option_trade_assets, db).await
}

#[instrument]
async fn invalidate_external_trade_draft_pick_options<C>(
    external_draft_pick_option_trade_assets: Vec<trade_asset::Model>,
    db: &C,
) -> Result<()>
where
    C: ConnectionTrait + Debug,
{
    let affected_draft_pick_option_ids = external_draft_pick_option_trade_assets.iter().map(|draft_pick_option_trade_asset| draft_pick_option_trade_asset.draft_pick_option_id.ok_or_else(|| eyre!("Couldn't get draft pick option id of supposed draft pick option trade asset (id = {})", draft_pick_option_trade_asset.id))).collect::<Result<Vec<i64>>>()?;
    draft_pick_option::Entity::update_many()
        .col_expr(
            draft_pick_option::Column::Status,
            Expr::value(DraftPickOptionStatus::InvalidatedByExternalTrade),
        )
        .filter(draft_pick_option::Column::Id.is_in(affected_draft_pick_option_ids))
        .exec(db)
        .await?;

    Ok(())
}
