use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use shared::responses::ApiResponse;
use thiserror::Error;

// Scheduling Service Error
#[derive(Debug, Error)]
pub enum SchedulingServiceError {
    #[error("Not Found: {0}")]
    NotFound(String),

    #[error("Bad Request: {0}")]
    BadRequest(String),

    #[error("Internal Server Error: {0}")]
    Internal(String),

    #[error("Database Error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Data Service Error: {0}")]
    DataService(String),
}

impl IntoResponse for SchedulingServiceError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message.clone()),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message.clone()),
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message.clone()),
            Self::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Oof, Something went wrong while accessing the database.".into(),
            ),
            Self::DataService(message) => (StatusCode::BAD_GATEWAY, message.clone()),
        };

        let body = ApiResponse::<()>::err(message);
        (status, axum::Json(body)).into_response()
    }
}
