use axum::{
    http::{Error as AxumError, StatusCode},
    response::{IntoResponse, Response},
};
use axum_sessions::async_session::{serde_json::Error as JsonError, Error as SessionError};
use fbkl_auth::{argon2::password_hash::Error as Argon2Error, hex::FromHexError};
use fbkl_entity::sea_orm::DbErr;
use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub enum FbklError {
    Axum(AxumError),
    Db(DbErr),
    HexStringConversion(FromHexError),
    Json(JsonError),
    PasswordHasher(Argon2Error),
    Session(SessionError),
}

impl Display for FbklError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Axum(err) => write!(fmt, "{}", err),
            Self::Db(err) => write!(fmt, "{}", err),
            Self::HexStringConversion(err) => write!(fmt, "{}", err),
            Self::Json(err) => write!(fmt, "{}", err),
            Self::PasswordHasher(err) => write!(fmt, "{}", err),
            Self::Session(err) => write!(fmt, "{}", err),
        }
    }
}

impl Error for FbklError {}

impl From<AxumError> for FbklError {
    fn from(err: AxumError) -> Self {
        FbklError::Axum(err)
    }
}

impl From<DbErr> for FbklError {
    fn from(err: DbErr) -> Self {
        FbklError::Db(err)
    }
}

impl From<FromHexError> for FbklError {
    fn from(err: FromHexError) -> Self {
        FbklError::HexStringConversion(err)
    }
}

impl From<JsonError> for FbklError {
    fn from(err: JsonError) -> Self {
        FbklError::Json(err)
    }
}

impl From<Argon2Error> for FbklError {
    fn from(err: Argon2Error) -> Self {
        FbklError::PasswordHasher(err)
    }
}

impl From<SessionError> for FbklError {
    fn from(err: SessionError) -> Self {
        FbklError::Session(err)
    }
}

impl IntoResponse for FbklError {
    fn into_response(self) -> Response {
        let body = match self {
            Self::Axum(err) => {
                format!("Web server encountered an error: {:?}", err)
            }
            Self::Db(err) => {
                format!("Database connection encountered an error: {:?}", err)
            }
            Self::HexStringConversion(err) => {
                format!("Tokenization encountered an error: {:?}", err)
            }
            Self::Json(err) => {
                format!("JSON0 conversion encountered an error: {:?}", err)
            }
            Self::PasswordHasher(err) => {
                format!("Password hasher encountered an error: {:?}", err)
            }
            Self::Session(err) => {
                format!("Couldn't retrieve/store user session: {:?}", err)
            }
        };

        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
