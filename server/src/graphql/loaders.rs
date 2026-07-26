//! `DataLoader`s for the per-field lookups that would otherwise fan out one query per row.
//!
//! A roster view resolves `Team.contracts -> Contract.leagueOrRealPlayer -> RealPlayer.position`,
//! so without batching a 15-contract roster costs dozens of round-trips. Registered once in
//! `build_graphql_schema`; field resolvers pull them out of the request context.

use std::{collections::HashMap, sync::Arc};

use async_graphql::dataloader::Loader;
use fbkl_entity::{
    league_player, league_player_queries::find_league_players_by_ids, player,
    player_queries::find_players_by_ids, position, position_queries::find_positions_by_ids,
    real_team, real_team_queries::find_real_teams_by_ids, sea_orm::DatabaseConnection,
};

/// `Loader::Error` must be `Clone`, and `color_eyre::Report` isn't.
type LoadError = Arc<color_eyre::Report>;

macro_rules! id_loader {
    ($loader:ident, $key:ty, $model:path, $fetch:path) => {
        pub struct $loader(pub DatabaseConnection);

        impl Loader<$key> for $loader {
            type Value = $model;
            type Error = LoadError;

            async fn load(&self, keys: &[$key]) -> Result<HashMap<$key, Self::Value>, Self::Error> {
                let models = $fetch(keys.to_vec(), &self.0).await.map_err(Arc::new)?;
                Ok(models.into_iter().map(|model| (model.id, model)).collect())
            }
        }
    };
}

id_loader!(PlayerLoader, i64, player::Model, find_players_by_ids);
id_loader!(
    LeaguePlayerLoader,
    i64,
    league_player::Model,
    find_league_players_by_ids
);
id_loader!(PositionLoader, i32, position::Model, find_positions_by_ids);
id_loader!(
    RealTeamLoader,
    i64,
    real_team::Model,
    find_real_teams_by_ids
);

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_graphql::dataloader::DataLoader;

    use super::*;

    /// Stands in for a real loader so the batching assertion needs no database.
    struct CountingLoader(Arc<AtomicUsize>);

    impl Loader<i64> for CountingLoader {
        type Value = i64;
        type Error = LoadError;

        async fn load(&self, keys: &[i64]) -> Result<HashMap<i64, i64>, Self::Error> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(keys.iter().map(|key| (*key, *key * 10)).collect())
        }
    }

    #[tokio::test]
    async fn concurrent_loads_batch_into_one_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let loader = DataLoader::new(CountingLoader(calls.clone()), tokio::spawn);

        let (first, second, third) =
            tokio::join!(loader.load_one(1), loader.load_one(2), loader.load_one(3));

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first.expect("load succeeds"), Some(10));
        assert_eq!(second.expect("load succeeds"), Some(20));
        assert_eq!(third.expect("load succeeds"), Some(30));
    }
}
