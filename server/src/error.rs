use axum::{
    http::{Error as AxumError, StatusCode},
    response::{IntoResponse, Response},
};
use tower_sessions::session::Error as SessionError;
use fbkl_auth::{argon2::password_hash::Error as Argon2Error, hex::FromHexError};
use fbkl_entity::sea_orm::DbErr;
use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub enum FbklError {
    Axum(AxumError),
    Db(DbErr),
    HexStringConversion(FromHexError),
    PasswordHasher(Argon2Error),
    Session(SessionError),
    StatusCode(StatusCode),
}

impl Display for FbklError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Axum(err) => write!(fmt, "{}", err),
            Self::Db(err) => write!(fmt, "{}", err),
            Self::HexStringConversion(err) => write!(fmt, "{}", err),
            Self::PasswordHasher(err) => write!(fmt, "{}", err),
            Self::Session(err) => write!(fmt, "{}", err),
            Self::StatusCode(err) => write!(fmt, "{}", err),
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

impl From<StatusCode> for FbklError {
    fn from(err: StatusCode) -> Self {
        FbklError::StatusCode(err)
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
            Self::PasswordHasher(err) => {
                format!("Password hasher encountered an error: {:?}", err)
            }
            Self::Session(err) => {
                format!("Couldn't retrieve/store user session: {:?}", err)
            }
            Self::StatusCode(err) => {
                return err.into_response();
            }
        };

        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
