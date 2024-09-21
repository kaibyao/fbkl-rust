use async_graphql::{Context, Object, Result};
use fbkl_entity::{
    player, position_queries::find_position_by_id, real_team_queries::find_real_team_by_id,
    sea_orm::DatabaseConnection,
};

#[derive(Clone, Default)]
pub struct Player {
    pub id: i64,
    pub name: String,
    pub photo_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub position_id: i32,
    pub position: String,
    pub real_team_id: i64,
    pub real_team_name: String,
    // TODO: GQL: Add contract info to player
}

impl Player {
    pub fn from_model(entity: player::Model) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            photo_url: entity.photo_url,
            thumbnail_url: entity.thumbnail_url,
            position_id: entity.position_id,
            position: "".to_string(),
            real_team_id: entity.current_real_team_id,
            real_team_name: "".to_string(),
        }
    }
}

#[Object]
impl Player {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn photo_url(&self) -> Option<String> {
        self.photo_url.clone()
    }

    async fn thumbnail_url(&self) -> Option<String> {
        self.thumbnail_url.clone()
    }

    async fn position_id(&self) -> i32 {
        self.position_id
    }

    async fn position(&self, ctx: &Context<'_>) -> Result<String> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let position = find_position_by_id(self.position_id, db).await?;
        Ok(position.name)
    }

    async fn real_team_id(&self) -> i64 {
        self.real_team_id
    }

    async fn real_team_name(&self, ctx: &Context<'_>) -> Result<String> {
        let db = ctx.data_unchecked::<DatabaseConnection>();

        let real_team = find_real_team_by_id(self.real_team_id, db).await?;
        Ok(real_team.name)
    }
}
