use std::fmt::Debug;

use color_eyre::Result;
use sea_orm::{ActiveModelTrait, ActiveValue, ConnectionTrait, TransactionTrait};
use tracing::instrument;

use crate::{
    team_trade,
    trade::{self, TradeStatus},
    trade_asset,
};

/// This is needed in order to set the `original_contract_id` after creating a new contract.
#[instrument]
pub async fn propose_new_trade<C>(
    league_id: i64,
    end_of_season_year: i16,
    from_team_id: i64,
    to_team_ids: &[i64],
    proposed_trade_assets: Vec<trade_asset::ActiveModel>,
    db: &C,
) -> Result<trade::Model>
where
    C: ConnectionTrait + TransactionTrait + Debug,
{
    let trade_model_to_insert = trade::ActiveModel {
        id: ActiveValue::NotSet,
        end_of_season_year: ActiveValue::Set(end_of_season_year),
        status: ActiveValue::Set(TradeStatus::Proposed),
        league_id: ActiveValue::Set(league_id),
        original_trade_id: ActiveValue::NotSet,
        previous_trade_id: ActiveValue::NotSet,
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    let inserted_trade = trade_model_to_insert.insert(db).await?;
    let inserted_trade_id = inserted_trade.id;

    let mut model_to_update: trade::ActiveModel = inserted_trade.into();
    model_to_update.original_trade_id = ActiveValue::Set(Some(inserted_trade_id));
    let updated_trade = model_to_update.update(db).await?;

    // insert team_trade records
    let mut all_team_ids = vec![];
    all_team_ids.push(from_team_id);
    all_team_ids.extend(to_team_ids);
    for team_id in all_team_ids {
        let team_trade_to_insert = team_trade::ActiveModel {
            id: ActiveValue::NotSet,
            team_id: ActiveValue::Set(team_id),
            trade_id: ActiveValue::Set(updated_trade.id),
        };
        let _inserted_team_trade_model = team_trade_to_insert.insert(db).await?;
    }

    for mut trade_asset_to_insert in proposed_trade_assets {
        trade_asset_to_insert.trade_id = ActiveValue::Set(updated_trade.id);
        let _inserted_trade_asset = trade_asset_to_insert.insert(db).await?;
    }

    Ok(updated_trade)
}