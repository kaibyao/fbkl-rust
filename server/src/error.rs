use std::fmt::Display;

use actix_web::error::{Error as ActixError, ResponseError};
use argon2::password_hash::Error as Argon2Error;
use db::diesel::r2d2::PoolError as R2D2Error;
use db::diesel::result::Error as DieselError;
use hex::FromHexError;

#[derive(Debug)]
pub enum ErrorType {
    Actix(ActixError),
    Diesel(DieselError),
    HexStringConversion(FromHexError),
    PasswordHasher(Argon2Error),
    R2D2(R2D2Error),
}

impl Display for ErrorType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Actix(actix_error) => write!(fmt, "{}", actix_error),
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

impl From<ActixError> for FbklError {
    fn from(err: ActixError) -> Self {
        FbklError(ErrorType::Actix(err))
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

impl ResponseError for FbklError {
    // fn error_response(&self) -> Response<Body> {

    // }
}
