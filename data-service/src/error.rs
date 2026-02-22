use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::responses::ApiResponse;
use thiserror::Error;

/// Application-level errors for the data service.
///
/// Each variant maps to an HTTP status code via the [`IntoResponse`] implementation.
#[derive(Debug, Error)]
pub enum DataServiceError {
    /// Requested resource was not found.
    #[error("Not Found: {0}")]
    NotFound(String),

    /// Operation conflicts with existing data (e.g., duplicate).
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Client sent an invalid request.
    #[error("Bad Request: {0}")]
    BadRequest(String),

    /// Unexpected internal failure.
    #[error("Internal Server Error: {0}")]
    Internal(String),

    /// Database query or connection error.
    #[error("Database Error: {0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for DataServiceError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        if status.is_server_error() {
            tracing::error!(error = %self, %status, "Server error");
        } else {
            tracing::warn!(error = %self, %status, "Client error");
        }

        let message = match self {
            Self::Database(_) => {
                "Oof, Something went wrong while accessing the database.".to_owned()
            }
            other => other.to_string(),
        };

        let body = ApiResponse::<()>::err(message);
        (status, axum::Json(body)).into_response()
    }
}
