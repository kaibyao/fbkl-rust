// use async_graphql::{Context, Object};

// #[derive(Default)]
// pub struct TeamQuery;

// #[Object]
// impl TeamQuery {
//     async fn teams(&self, ctx: &Context<'_>) -> Result<Vec<Team>> {
//         let user_model = match ctx.data_unchecked::<Option<user::Model>>().to_owned() {
//             None => return Ok(vec![]),
//             Some(user) => user,
//         };
//         let db = ctx.data_unchecked::<DatabaseConnection>();
//         let team_models = find_teams_by_user(&user_model, db).await?;

//         let teams = team_models.into_iter().map(Team::from_model).collect();
//         Ok(teams)
//     }

//     async fn team(&self, ctx: &Context<'_>) -> Result<Team, FbklError> {
//         let session = ctx.data_unchecked::<Session>();
//         let selected_team_id: i64 = match session.get("selected_team_id").await? {
//             None => return Err(StatusCode::BAD_REQUEST.into()),
//             Some(id) => id,
//         };

//         let user_model = match ctx.data_unchecked::<Option<user::Model>>().to_owned() {
//             None => return Err(StatusCode::UNAUTHORIZED.into()),
//             Some(user) => user,
//         };
//         let db = ctx.data_unchecked::<DatabaseConnection>();

//         match find_team_by_user(&user_model, selected_team_id, db).await? {
//             None => Err(StatusCode::NOT_FOUND.into()),
//             Some(team_model) => Ok(Team::from_model(team_model)),
//         }
//     }
// }
