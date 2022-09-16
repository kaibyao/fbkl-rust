use std::error::Error;
use std::fmt::Display;
use actix_web::error::{Error as ActixError, ResponseError};
use actix_web::http::StatusCode;
use axum::Error as AxumError;
use axum::response::{IntoResponse, Response};
use fbkl_auth::{argon2::password_hash::Error as Argon2Error, hex::FromHexError};
use fbkl_db::diesel::r2d2::PoolError as R2D2Error;
use fbkl_db::diesel::result::Error as DieselError;

#[derive(Debug)]
pub enum ErrorType {
    Actix(ActixError),
    Axum(AxumError),
    Diesel(DieselError),
    HexStringConversion(FromHexError),
    PasswordHasher(Argon2Error),
    R2D2(R2D2Error),
}

impl Display for ErrorType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Actix(actix_error) => write!(fmt, "{}", actix_error),
            Self::Axum(axum_error) => write!(fmt, "{}", axum_error),
            Self::Diesel(diesel_error) => write!(fmt, "{}", diesel_error),
            Self::HexStringConversion(hex_error) => write!(fmt, "{}", hex_error),
            Self::PasswordHasher(argon2_error) => write!(fmt, "{}", argon2_error),
            Self::R2D2(r2d2_error) => write!(fmt, "{}", r2d2_error),
        }
    }
}

#[derive(Debug)]
pub struct FbklError(ErrorType);

impl Display for FbklError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.0)
    }
}

impl Error for FbklError {}

impl From<ActixError> for FbklError {
    fn from(err: ActixError) -> Self {
        FbklError(ErrorType::Actix(err))
    }
}

impl From<AxumError> for FbklError {
    fn from(err: AxumError) -> Self {
        FbklError(ErrorType::Axum(err))
    }
}

impl From<DieselError> for FbklError {
    fn from(err: DieselError) -> Self {
        FbklError(ErrorType::Diesel(err))
    }
}

impl From<FromHexError> for FbklError {
    fn from(err: FromHexError) -> Self {
        FbklError(ErrorType::HexStringConversion(err))
    }
}

impl From<Argon2Error> for FbklError {
    fn from(err: Argon2Error) -> Self {
        FbklError(ErrorType::PasswordHasher(err))
    }
}

impl From<R2D2Error> for FbklError {
    fn from(err: R2D2Error) -> Self {
        FbklError(ErrorType::R2D2(err))
    }
}

impl IntoResponse for FbklError {
    fn into_response(self) -> Response {
        let body = match self.0 {
            ErrorType::Actix(actix_error) => format!("Web server encountered an error: {:?}", actix_error),
            ErrorType::Axum(axum_error) => format!("Web server encountered an error: {:?}", axum_error),
            ErrorType::Diesel(diesel_error) => format!("Database connection encountered an error: {:?}", diesel_error),
            ErrorType::HexStringConversion(hex_error) => format!("Tokenization encountered an error: {:?}", hex_error),
            ErrorType::PasswordHasher(argon2_error) => format!("Password hasher encountered an error: {:?}", argon2_error),
            ErrorType::R2D2(r2d2_error) => format!("Database pool encountered an error: {:?}", r2d2_error),
        };

        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

impl ResponseError for FbklError {
    // fn error_response(&self) -> Response<Body> {

    // }
}
