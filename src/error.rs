use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::error::Error;

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("resource not found: {message}")]
    NotFound { message: String },

    #[error("game was modified concurrently")]
    Conflict,

    #[error("persisted game data is invalid: {message}")]
    InvalidData { message: String },

    #[error("persisted game state is invalid: {message}")]
    InvalidState { message: String },

    #[error("operation invalid for current game status: {message}")]
    InvalidStatus { message: String },

    #[error("repository operation failed")]
    Unexpected {
        #[source]
        source: BoxError,
    },
}
impl AppError {
    pub(crate) fn invalid_data(error: impl std::fmt::Display) -> Self {
        Self::InvalidData {
            message: error.to_string(),
        }
    }
    pub(crate) fn unexpected(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Unexpected {
            source: Box::new(error),
        }
    }

    /// The status code and the message the client is allowed to see.
    fn public(&self) -> (StatusCode, &str) {
        match self {
            Self::NotFound { message } => (StatusCode::NOT_FOUND, message),
            Self::Conflict => (StatusCode::CONFLICT, "game was modified concurrently"),
            Self::InvalidStatus { message } => (StatusCode::BAD_REQUEST, message),
            Self::InvalidState { message } => (StatusCode::BAD_REQUEST, message),
            Self::InvalidData { message } => (StatusCode::BAD_REQUEST, message),
            Self::Unexpected { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error"),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

// lets `?` convert
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Unexpected {
            source: Box::new(e),
        }
    }
}

// lets axum turn it into a response
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error) = self.public();

        if status.is_server_error() {
            eprintln!("{self:?}");
        }

        (status, Json(ErrorBody { error })).into_response()
    }
}
