use fbkl_entity::{
    position, real_team,
    sea_orm::{ActiveValue, DatabaseConnection, EntityTrait},
};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        generate_positions(db).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .truncate_table(
                TableTruncateStatement::default()
                    .table(real_team::Entity)
                    .to_owned(),
            )
            .await
    }
}

async fn generate_positions(db: &DatabaseConnection) -> Result<(), DbErr> {
    let espn_position_ids: [(i16, &str); 16] = [
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

    let models: Vec<position::ActiveModel> = espn_position_ids
        .into_iter()
        .map(|(espn_id, name)| position::ActiveModel {
            espn_id: ActiveValue::Set(espn_id),
            name: ActiveValue::Set(name.to_string()),
            ..Default::default()
        })
        .collect();

    position::Entity::insert_many(models).exec(db).await?;

    Ok(())
}
