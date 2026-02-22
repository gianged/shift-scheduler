use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Standard JSON response envelope used by both services.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Creates a success response wrapping the given data.
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Creates an error response with the given message.
    pub fn err(error_msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error_msg.into()),
        }
    }
}

/// Response envelope for operations that return no data.
#[derive(Debug, Serialize, ToSchema)]
pub struct EmptyApiResponse {
    pub success: bool,
    pub error: Option<String>,
}

/// Response for the `/headpat` health check endpoint.
#[derive(Debug, Serialize, ToSchema)]
pub struct HeadpatResponse {
    pub message: &'static str,
}
