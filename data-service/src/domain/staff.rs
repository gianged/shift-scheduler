use async_trait::async_trait;
use serde::Deserialize;
use shared::types::{Staff, StaffStatus};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::DataServiceError;

/// Request payload for creating a new staff member.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateStaff {
    pub name: String,
    pub email: String,
    pub position: String,
}

/// Request payload for partially updating an existing staff member. All fields are optional.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateStaff {
    pub name: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub status: Option<StaffStatus>,
}

/// Persistence operations for staff members.
#[cfg_attr(feature = "test-support", mockall::automock)]
#[async_trait]
pub trait StaffRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Staff>, DataServiceError>;
    async fn find_all(&self) -> Result<Vec<Staff>, DataServiceError>;
    async fn create(&self, staff: CreateStaff) -> Result<Staff, DataServiceError>;
    async fn batch_create(&self, staffs: Vec<CreateStaff>) -> Result<Vec<Staff>, DataServiceError>;
    async fn update(&self, id: Uuid, staff: UpdateStaff) -> Result<Staff, DataServiceError>;
    async fn deactivate(&self, id: Uuid) -> Result<(), DataServiceError>;
    async fn delete(&self, id: Uuid) -> Result<(), DataServiceError>;
}
