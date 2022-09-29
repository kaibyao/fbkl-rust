use async_graphql::{EmptyMutation, EmptySubscription, MergedObject, Schema};

use self::user::UserQuery;

mod user;

pub type FbklSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

#[derive(Default, MergedObject)]
pub struct QueryRoot(UserQuery);
