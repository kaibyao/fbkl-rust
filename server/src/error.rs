use axum::{
    http::{Error as AxumError, StatusCode},
    response::{IntoResponse, Response},
};
use fbkl_auth::{argon2::password_hash::Error as Argon2Error, hex::FromHexError};
use fbkl_entity::sea_orm::DbErr;
use thiserror::Error;
use tower_sessions::session::Error as SessionError;

#[derive(Debug, Error)]
pub enum FbklError {
    #[error("web server error")]
    Axum(#[from] AxumError),
    #[error("database error")]
    Db(#[from] DbErr),
    #[error("invalid hex string")]
    HexStringConversion(#[from] FromHexError),
    #[error("password hashing error")]
    PasswordHasher(#[from] Argon2Error),
    #[error("session error")]
    Session(#[from] SessionError),
    // Explicit client-facing status code (replaces the old `StatusCode` variant).
    #[error("request failed: {0}")]
    Status(StatusCode),
}

impl From<StatusCode> for FbklError {
    fn from(code: StatusCode) -> Self {
        Self::Status(code)
    }
}

impl FbklError {
    /// Maps each error variant to the HTTP status the client should see.
    ///
    /// Client-caused faults (bad hex tokens, wrong password) are 4xx; everything
    /// the server is responsible for is 5xx.
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::HexStringConversion(_) | Self::PasswordHasher(_) => StatusCode::BAD_REQUEST,
            Self::Status(code) => *code,
            Self::Axum(_) | Self::Db(_) | Self::Session(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
