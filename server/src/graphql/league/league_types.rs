use async_graphql::Object;
// use fbkl_entity::league;

#[derive(Default)]
pub struct League {
    pub id: i64,
    pub name: String,
    // pub teams: Vec<>,
    // pub users: Vec<>,
}

// impl League {
//     pub fn from_model(entity: league::Model) -> Self {
//         Self {
//             id: entity.id,
//             name: entity.name,
//         }
//     }
// }

#[Object]
impl League {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn name(&self) -> String {
        self.name.clone()
    }
}
