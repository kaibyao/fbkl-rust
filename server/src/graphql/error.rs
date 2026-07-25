//! Stable GraphQL error codes.
//!
//! Every error returned from a resolver carries a machine-readable `code`
//! extension so the frontend can switch on the failure instead of parsing
//! message strings. Domain-validation failures (keeper limits, chain staleness,
//! not-on-the-clock) get their own code and never surface as a bare 500.

use async_graphql::{Error as GraphQlError, ErrorExtensions};
use axum::http::StatusCode;

use crate::error::FbklError;

/// Machine-readable value of the `code` error extension.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    /// Not logged in.
    Unauthenticated,
    /// Logged in, but not allowed to do this.
    Forbidden,
    /// Asset does not exist (or is not visible in the selected league).
    NotFound,
    /// Client sent an unusable argument or has no league selected.
    BadRequest,
    /// Acting on a trade/contract that a newer record in its chain has superseded.
    NotLatestInChain,
    /// A team involved in a trade has no pre-trade salary snapshot, so it cannot be processed.
    MissingPreTradeSalary,
    /// Server-side fault; message is deliberately generic.
    Internal,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unauthenticated => "UNAUTHENTICATED",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::BadRequest => "BAD_REQUEST",
            Self::NotLatestInChain => "NOT_LATEST_IN_CHAIN",
            Self::MissingPreTradeSalary => "MISSING_PRE_TRADE_SALARY",
            Self::Internal => "INTERNAL",
        }
    }

    const fn default_message(self) -> &'static str {
        match self {
            Self::Unauthenticated => "not logged in",
            Self::Forbidden => "not allowed",
            Self::NotFound => "not found",
            Self::BadRequest => "bad request",
            Self::NotLatestInChain => "a newer version of this record supersedes it",
            Self::MissingPreTradeSalary => {
                "a team involved in this trade is missing its pre-trade salary"
            }
            Self::Internal => "internal server error",
        }
    }
}

/// Build a GraphQL error carrying `code` plus a caller-supplied message.
pub fn graphql_error(code: ErrorCode, message: impl Into<String>) -> GraphQlError {
    GraphQlError::new(message).extend_with(|_, ext| ext.set("code", code.as_str()))
}

/// Build a GraphQL error carrying `code` and its generic message.
pub fn code_error(code: ErrorCode) -> GraphQlError {
    graphql_error(code, code.default_message())
}

/// Convert an `FbklError` into a coded GraphQL error, logging server faults.
pub fn from_fbkl(error: &FbklError) -> GraphQlError {
    let status = error.status_code();
    let code = match status {
        StatusCode::UNAUTHORIZED => ErrorCode::Unauthenticated,
        StatusCode::FORBIDDEN => ErrorCode::Forbidden,
        StatusCode::NOT_FOUND => ErrorCode::NotFound,
        _ if status.is_client_error() => ErrorCode::BadRequest,
        _ => ErrorCode::Internal,
    };

    if code == ErrorCode::Internal {
        tracing::error!(error = ?error, "graphql resolver failed");
        return code_error(code);
    }

    graphql_error(code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_attached_as_an_extension() {
        let error = code_error(ErrorCode::Forbidden);
        let code = error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .cloned();

        assert_eq!(code, Some("FORBIDDEN".into()));
        assert_eq!(error.message, "not allowed");
    }

    #[test]
    fn fbkl_server_faults_are_coded_internal_and_not_leaked() {
        let error = from_fbkl(&FbklError::from(StatusCode::INTERNAL_SERVER_ERROR));

        assert_eq!(error.message, "internal server error");
        assert_eq!(
            error.extensions.as_ref().and_then(|ext| ext.get("code")),
            Some(&"INTERNAL".into())
        );
    }

    #[test]
    fn fbkl_client_faults_keep_their_message() {
        let error = from_fbkl(&FbklError::BadRequest("bad team id".to_owned()));

        assert_eq!(error.message, "bad team id");
        assert_eq!(
            error.extensions.as_ref().and_then(|ext| ext.get("code")),
            Some(&"BAD_REQUEST".into())
        );
    }
}
