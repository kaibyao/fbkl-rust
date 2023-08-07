use fbkl_entity::{
    player, position,
    sea_orm::{ActiveValue, ColumnTrait, EntityTrait},
};
use sea_orm_migration::{prelude::*, sea_orm::QueryFilter};

#[derive(DeriveMigrationName)]
pub struct Migration;

static ESPN_POSITION_IDS: [(i16, &str); 16] = [
    (0, "PG"),
    (1, "SG"),
    (2, "SF"),
    (3, "PF"),
    (4, "C"),
    (5, "G"),
    (6, "F"),
    (7, "SG/SF"),
    (8, "G/F"),
    (9, "PF/C"),
    (10, "F/C"),
    (11, "UT"),
    (12, "BE"),
    (13, "IR"),
    (14, ""),
    (15, "Rookie"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        generate_positions(db).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let espn_position_ids: Vec<i16> = ESPN_POSITION_IDS
            .iter()
            .map(|(id, _)| id.to_owned())
            .collect();

        player::Entity::delete_many()
            .exec(manager.get_connection())
            .await?;

        position::Entity::delete_many()
            .filter(position::Column::EspnId.is_in(espn_position_ids))
            .exec(manager.get_connection())
            .await
            .map(|_| ())
    }
}

async fn generate_positions(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let models: Vec<position::ActiveModel> = ESPN_POSITION_IDS
        .iter()
        .map(|(espn_id, name)| position::ActiveModel {
            espn_id: ActiveValue::Set(*espn_id),
            name: ActiveValue::Set(name.to_string()),
            ..Default::default()
        })
        .collect();

    position::Entity::insert_many(models).exec(db).await?;

    Ok(())
}
