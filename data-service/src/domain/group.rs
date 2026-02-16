use async_trait::async_trait;
use serde::Deserialize;
use shared::types::StaffGroup;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::DataServiceError;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroup {
    pub name: String,
    pub parent_group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateGroup {
    pub name: Option<String>,
    #[schema(nullable)]
    pub parent_group_id: Option<Option<Uuid>>,
}

#[cfg_attr(feature = "test-support", mockall::automock)]
#[async_trait]
pub trait GroupRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<StaffGroup>, DataServiceError>;
    async fn find_all(&self) -> Result<Vec<StaffGroup>, DataServiceError>;
    async fn create(&self, group: CreateGroup) -> Result<StaffGroup, DataServiceError>;
    async fn batch_create(
        &self,
        groups: Vec<CreateGroup>,
    ) -> Result<Vec<StaffGroup>, DataServiceError>;
    async fn update(&self, id: Uuid, group: UpdateGroup) -> Result<StaffGroup, DataServiceError>;
    async fn delete(&self, id: Uuid) -> Result<(), DataServiceError>;
}
