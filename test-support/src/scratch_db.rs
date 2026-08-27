//! Per-test scratch database creation, i.e. the harness plumbing that has nothing to do with
//! fantasy basketball.

use fbkl_entity::sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use fbkl_migration::{Migrator, MigratorTrait};

/// Drops and recreates `fbkl_test_<test_name>` next to `DATABASE_URL`, then migrates it.
pub async fn scratch_db(test_name: &str) -> Option<DatabaseConnection> {
    dotenvy::dotenv().ok();
    let Ok(base_url) = std::env::var("DATABASE_URL") else {
        assert!(
            std::env::var_os("CI").is_none(),
            "{test_name} needs a database, and CI set no DATABASE_URL: the whole DB suite would pass without running"
        );
        eprintln!("skipping {test_name}: DATABASE_URL not set");
        return None;
    };

    let (host_url, _) = base_url
        .trim_end_matches('/')
        .rsplit_once('/')
        .expect("DATABASE_URL must end in a database name");
    let scratch_name = format!("fbkl_test_{test_name}");

    let admin_db = Database::connect(format!("{host_url}/postgres"))
        .await
        .expect("connect to the postgres maintenance database");
    admin_db
        .execute_unprepared(&format!(
            "DROP DATABASE IF EXISTS {scratch_name} WITH (FORCE)"
        ))
        .await
        .expect("drop scratch database");
    admin_db
        .execute_unprepared(&format!("CREATE DATABASE {scratch_name}"))
        .await
        .expect("create scratch database");

    let db = Database::connect(format!("{host_url}/{scratch_name}"))
        .await
        .expect("connect to scratch database");
    Migrator::up(&db, None)
        .await
        .expect("migrate scratch database");
    Some(db)
}
