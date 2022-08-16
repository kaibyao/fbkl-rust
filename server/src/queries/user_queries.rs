use color_eyre::Result;
use db::models::User;

// I need to implement a struct like they do here: https://github.com/diesel-rs/diesel/blob/master/examples/postgres/all_about_inserts/src/lib.rs#L27

pub async fn insert(user: User) -> Result<i64> {
    Ok(0)
}
