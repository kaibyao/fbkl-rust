use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, JoinType, QueryFilter, QuerySelect,
    RelationTrait, TransactionTrait,
};

use crate::team;

pub async fn find_teams_by_league<C>(league_id: i64, db: &C) -> Result<Vec<team::Model>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    team::Entity::find()
        .join(JoinType::LeftJoin, team::Relation::League.def())
        .filter(team::Column::LeagueId.eq(league_id))
        .all(db)
        .await
}
