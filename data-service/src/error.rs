use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use shared::responses::ApiResponse;
use thiserror::Error;

// Data Service Error
#[derive(Debug, Error)]
pub enum DataServiceError {
    #[error("Not Found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Bad Request: {0}")]
    BadRequest(String),

    #[error("Internal Server Error: {0}")]
    Internal(String),

    #[error("Database Error: {0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for DataServiceError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message.clone()),
            Self::Conflict(message) => (StatusCode::CONFLICT, message.clone()),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message.clone()),
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message.clone()),
            Self::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Oof, Something went wrong while accessing the database.".into(),
            ),
        };

        if status.is_server_error() {
            tracing::error!(error = %self, %status, "Server error");
        } else {
            tracing::warn!(error = %self, %status, "Client error");
        }

        let body = ApiResponse::<()>::err(message);
        (status, axum::Json(body)).into_response()
    }
}
