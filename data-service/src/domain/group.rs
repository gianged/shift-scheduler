use async_trait::async_trait;
use serde::Deserialize;
use shared::types::StaffGroups;
use uuid::Uuid;

use crate::error::DataServiceError;

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    pub parent_group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    pub name: Option<String>,
    pub parent_group_id: Option<Option<Uuid>>,
}

#[async_trait]
pub trait GroupRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<StaffGroups>, DataServiceError>;
    async fn find_all(&self) -> Result<Vec<StaffGroups>, DataServiceError>;
    async fn create(&self, group: CreateGroup) -> Result<StaffGroups, DataServiceError>;
    async fn batch_create(
        &self,
        groups: Vec<CreateGroup>,
    ) -> Result<Vec<StaffGroups>, DataServiceError>;
    async fn update(&self, id: Uuid, group: UpdateGroup) -> Result<StaffGroups, DataServiceError>;
    async fn delete(&self, id: Uuid) -> Result<(), DataServiceError>;
}
