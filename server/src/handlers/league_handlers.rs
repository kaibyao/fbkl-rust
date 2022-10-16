use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_sessions::extractors::WritableSession;
use fbkl_entity::league_queries::find_league_by_user;
use serde_json::Value;

use crate::{server::AppState, session::get_current_user_writable};

pub async fn select_league(
    State(state): State<Arc<AppState>>,
    mut write_session: WritableSession,
    Json(payload): Json<Value>,
) -> Result<Response, Response> {
    let user_model = match get_current_user_writable(&write_session, &state.db).await {
        None => return Err(StatusCode::UNAUTHORIZED.into_response()),
        Some(model) => model,
    };

    // parse league id from json
    let league_id: i64 = match payload.as_object() {
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Requires a json object with parameter 'leagueId'",
            )
                .into_response())
        }
        Some(league_json) => match league_json.get("leagueId") {
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Missing object attribute 'leagueId'",
                )
                    .into_response())
            }
            Some(id_json) => match id_json.as_i64() {
                None => {
                    return Err(
                        (StatusCode::BAD_REQUEST, "Could not parse 'leagueId' value")
                            .into_response(),
                    )
                }
                Some(id) => id,
            },
        },
    };

    // verify user has access to league
    match find_league_by_user(&user_model, league_id, &state.db).await {
        Err(_db_err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not connect with database.",
        )
            .into_response()),
        Ok(None) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Could not find league.").into_response())
        }
        // write league id to session
        Ok(Some(_league_model)) => match write_session.insert("selected_league_id", league_id) {
            Err(_) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not select league.",
            )
                .into_response()),
            Ok(_) => Ok(StatusCode::OK.into_response()),
        },
    }
}
