use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use shared::responses::ApiResponse;
use thiserror::Error;

/// Application-level errors for the scheduling service.
///
/// Each variant maps to an HTTP status code via the [`IntoResponse`] implementation.
#[derive(Debug, Error)]
pub enum SchedulingServiceError {
    /// Requested resource was not found.
    #[error("Not Found: {0}")]
    NotFound(String),

    /// Client sent an invalid request.
    #[error("Bad Request: {0}")]
    BadRequest(String),

    /// Unexpected internal failure.
    #[error("Internal Server Error: {0}")]
    Internal(String),

    /// Database query or connection error.
    #[error("Database Error: {0}")]
    Database(#[from] sqlx::Error),

    /// Data service returned a non-success response.
    #[error("Data Service Error: {0}")]
    DataService(String),

    /// Data service is unreachable after retries (connection-level failure).
    #[error("Data Service unavailable: {0}")]
    DataServiceUnavailable(String),

    /// Circuit breaker is open; data service calls are being fast-failed.
    #[error("Circuit breaker is open - data service unavailable")]
    CircuitOpen,
}

impl IntoResponse for SchedulingServiceError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DataService(_) | Self::DataServiceUnavailable(_) => StatusCode::BAD_GATEWAY,
            Self::CircuitOpen => StatusCode::SERVICE_UNAVAILABLE,
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
            Self::CircuitOpen => {
                "Data service is currently unavailable, please try again later".to_owned()
            }
            other => other.to_string(),
        };

        let body = ApiResponse::<()>::err(message);
        (status, axum::Json(body)).into_response()
    }
}
