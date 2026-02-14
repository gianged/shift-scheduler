use async_trait::async_trait;
use shared::types::Staff;
use uuid::Uuid;

use crate::error::SchedulingServiceError;

#[async_trait]
pub trait DataServiceClient: Send + Sync {
    async fn get_resolved_members(
        &self,
        staff_group_id: Uuid,
    ) -> Result<Vec<Staff>, SchedulingServiceError>;
}
