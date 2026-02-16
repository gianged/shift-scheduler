use async_trait::async_trait;
use serde::Deserialize;
use shared::types::{Staff, StaffGroup};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::DataServiceError;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMembership {
    pub staff_id: Uuid,
    pub group_id: Uuid,
}

#[cfg_attr(feature = "test-support", mockall::automock)]
#[async_trait]
pub trait MembershipRepository: Send + Sync {
    async fn add_staff_to_group(
        &self,
        group_id: Uuid,
        staff_id: Uuid,
    ) -> Result<(), DataServiceError>;
    async fn remove_staff_from_group(
        &self,
        group_id: Uuid,
        staff_id: Uuid,
    ) -> Result<(), DataServiceError>;
    async fn get_group_members(&self, group_id: Uuid) -> Result<Vec<Staff>, DataServiceError>;
    async fn get_staff_groups(&self, staff_id: Uuid) -> Result<Vec<StaffGroup>, DataServiceError>;
    async fn resolve_members(&self, group_id: Uuid) -> Result<Vec<Staff>, DataServiceError>;
    async fn batch_add_members(
        &self,
        memberships: Vec<AddMembership>,
    ) -> Result<(), DataServiceError>;
}
