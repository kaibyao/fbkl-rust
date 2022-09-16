pub use chrono;
pub use diesel;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};

pub mod models;
pub mod queries;
pub mod schema;

pub type FbklPool = Pool<ConnectionManager<PgConnection>>;

pub fn create_pool<S>(database_url: S) -> FbklPool
where
    S: Into<String>,
{
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .build(manager)
        .expect("Failed to create pool.")
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         let result = 2 + 2;
//         assert_eq!(result, 4);
//     }
// }
