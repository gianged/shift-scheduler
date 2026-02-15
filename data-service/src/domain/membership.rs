use async_trait::async_trait;
use shared::types::{Staff, StaffGroup};
use uuid::Uuid;

use crate::error::DataServiceError;

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
}
