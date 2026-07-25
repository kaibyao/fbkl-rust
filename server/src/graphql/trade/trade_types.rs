use async_graphql::{ComplexObject, Context, InputObject, Result, SimpleObject};
use fbkl_entity::{
    sea_orm::DatabaseConnection,
    trade::{self, TradeStatus},
    trade_action, trade_asset,
    trade_asset::TradeAssetType,
};

use crate::graphql::{ErrorCode, code_error, team::Team};

/// A trade between two or more teams. `status` drives what the client may still do with it; the
/// chain ids link counteroffers back to the original proposal.
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Trade {
    pub id: i64,
    pub end_of_season_year: i16,
    pub status: TradeStatus,
    pub league_id: i64,
    pub original_trade_id: Option<i64>,
    pub previous_trade_id: Option<i64>,
    pub transaction_id: Option<i64>,
    pub created_at: String,
    #[graphql(skip)]
    model: trade::Model,
}

impl Trade {
    pub(super) fn from_model(entity: trade::Model) -> Self {
        Self {
            id: entity.id,
            end_of_season_year: entity.end_of_season_year,
            status: entity.status,
            league_id: entity.league_id,
            original_trade_id: entity.original_trade_id,
            previous_trade_id: entity.previous_trade_id,
            transaction_id: entity.transaction_id,
            created_at: entity.created_at.to_string(),
            model: entity,
        }
    }
}

#[ComplexObject]
impl Trade {
    /// Every team involved, via `team_trade`.
    async fn teams(&self, ctx: &Context<'_>) -> Result<Vec<Team>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let team_models = self.model.get_teams(db).await.map_err(|err| {
            tracing::error!(error = ?err, trade_id = self.id, "failed to load trade teams");
            code_error(ErrorCode::Internal)
        })?;

        Ok(team_models.into_iter().map(Team::from_model).collect())
    }

    /// The assets on offer, each with the team sending and receiving it.
    async fn assets(&self, ctx: &Context<'_>) -> Result<Vec<TradeAsset>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let asset_models = self.model.get_trade_assets(db).await.map_err(|err| {
            tracing::error!(error = ?err, trade_id = self.id, "failed to load trade assets");
            code_error(ErrorCode::Internal)
        })?;

        Ok(asset_models.iter().map(TradeAsset::from_model).collect())
    }

    /// Proposal / accept / reject / counter history, oldest first.
    async fn actions(&self, ctx: &Context<'_>) -> Result<Vec<TradeAction>> {
        let db = ctx.data_unchecked::<DatabaseConnection>();
        let action_models = self.model.get_trade_actions(db).await.map_err(|err| {
            tracing::error!(error = ?err, trade_id = self.id, "failed to load trade actions");
            code_error(ErrorCode::Internal)
        })?;

        Ok(action_models.iter().map(TradeAction::from_model).collect())
    }
}

/// One asset moving between two teams. Exactly one of the three id fields is set, per `assetType`.
#[derive(SimpleObject)]
pub struct TradeAsset {
    pub id: i64,
    pub asset_type: TradeAssetType,
    pub contract_id: Option<i64>,
    pub draft_pick_id: Option<i64>,
    pub draft_pick_option_id: Option<i64>,
    pub from_team_id: i64,
    pub to_team_id: i64,
}

impl TradeAsset {
    const fn from_model(entity: &trade_asset::Model) -> Self {
        Self {
            id: entity.id,
            asset_type: entity.asset_type,
            contract_id: entity.contract_id,
            draft_pick_id: entity.draft_pick_id,
            draft_pick_option_id: entity.draft_pick_option_id,
            from_team_id: entity.from_team_id,
            to_team_id: entity.to_team_id,
        }
    }
}

/// One response a team made to a trade.
#[derive(SimpleObject)]
pub struct TradeAction {
    pub id: i64,
    pub action_type: trade_action::TradeActionType,
    pub team_user_id: i64,
    pub trade_id: i64,
    pub created_at: String,
}

impl TradeAction {
    fn from_model(entity: &trade_action::Model) -> Self {
        Self {
            id: entity.id,
            action_type: entity.action_type,
            team_user_id: entity.team_user_id,
            trade_id: entity.trade_id,
            created_at: entity.created_at.to_string(),
        }
    }
}

/// A trade proposal. `fromTeamId` must be the caller's own team; each group lists the assets the
/// named team *receives*. The sending team of every asset is derived from the database, never here.
#[derive(InputObject)]
pub struct ProposeTradeInput {
    pub from_team_id: i64,
    pub to_teams: Vec<ProposeTradeTeamInput>,
}

#[derive(InputObject)]
pub struct ProposeTradeTeamInput {
    pub to_team_id: i64,
    pub assets: Vec<ProposeTradeAssetInput>,
}

#[derive(InputObject)]
pub struct ProposeTradeAssetInput {
    pub asset_type: TradeAssetType,
    /// Id of the contract / draft pick / draft pick option, matching `assetType`.
    pub asset_id: i64,
}
