use async_graphql::Object;
use fbkl_entity::user;

#[derive(Clone, Default)]
pub struct User {
    pub id: i64,
    pub email: String,
}

impl User {
    pub fn from_model(entity: user::Model) -> Self {
        Self {
            id: entity.id,
            email: entity.email,
        }
    }
}

#[Object]
impl User {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn email(&self) -> String {
        self.email.clone()
    }
}
