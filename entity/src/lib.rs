pub extern crate sea_orm;

mod entities;
pub use entities::*;

mod queries;
pub use queries::*;

use sea_orm::{Linked, Related, RelationDef, RelationTrait};

// Complex foreign keys / relations

impl Related<team::Entity> for user::Entity {
    // The final relation is User -> TeamUser -> Team
    fn to() -> RelationDef {
        team_user::Relation::Team.def()
    }

    fn via() -> Option<RelationDef> {
        // The original relation is TeamUser -> User,
        // after `rev` it becomes User -> TeamUser
        Some(team_user::Relation::User.def().rev())
    }
}

impl Related<user::Entity> for team::Entity {
    // The final relation is Team -> TeamUser -> User
    fn to() -> RelationDef {
        team_user::Relation::User.def()
    }

    fn via() -> Option<RelationDef> {
        // The original relation is TeamUser -> Team,
        // after `rev` it becomes Team -> TeamUser
        Some(team_user::Relation::Team.def().rev())
    }
}

impl Linked for team_user::Entity {
    type FromEntity = user::Entity;
    type ToEntity = league::Entity;

    fn link(&self) -> Vec<RelationDef> {
        // league -> team -> team_user -> user
        vec![
            team_user::Relation::User.def().rev(),
            team_user::Relation::Team.def(),
            team::Relation::League.def(),
        ]
    }
}
