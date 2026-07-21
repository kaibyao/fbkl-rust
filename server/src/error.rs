use axum::{
    http::{Error as AxumError, StatusCode},
    response::{IntoResponse, Response},
};
use color_eyre::Report;
use fbkl_auth::AuthError;
use fbkl_entity::sea_orm::DbErr;
use thiserror::Error;
use tower_sessions::session::Error as SessionError;

#[derive(Debug, Error)]
pub enum FbklError {
    #[error("web server error")]
    Axum(#[from] AxumError),
    #[error("database error")]
    Db(#[from] DbErr),
    #[error("auth error")]
    Auth(#[from] AuthError),
    #[error("session error")]
    Session(#[from] SessionError),
    // Redacted `color_eyre::Report`; full chain logged at conversion, never sent to client.
    #[error("internal server error")]
    Internal(Report),
    // Client-safe 400 message (a bad input value the client itself sent).
    #[error("{0}")]
    BadRequest(String),
    #[error("request failed: {0}")]
    Status(StatusCode),
}

impl From<StatusCode> for FbklError {
    fn from(code: StatusCode) -> Self {
        Self::Status(code)
    }
}

impl From<Report> for FbklError {
    fn from(report: Report) -> Self {
        tracing::error!(error = ?report, "internal error");
        Self::Internal(report)
    }
}

impl FbklError {
    /// Maps each error variant to the HTTP status the client should see.
    ///
    /// Client-caused faults (bad hex tokens, wrong password) are 4xx; everything
    /// the server is responsible for is 5xx.
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::Auth(_) | Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Status(code) => *code,
            Self::Axum(_) | Self::Db(_) | Self::Session(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

impl IntoResponse for FbklError {
    fn into_response(self) -> Response {
        let status = self.status_code();

        // Log full detail server-side; never leak internals to the client.
        if status.is_server_error() {
            tracing::error!(error = ?self, "request failed");
        }

        // A bare status carries its own response; anything else gets a generic,
        // detail-free body so we don't disclose internal error contents.
        if let Self::Status(code) = self {
            code.into_response()
        } else {
            let body = status.canonical_reason().unwrap_or("error").to_string();
            (status, body).into_response()
        }
    }
}
