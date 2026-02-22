use async_trait::async_trait;
use shared::types::Staff;
use uuid::Uuid;

use crate::error::SchedulingServiceError;

/// Abstraction over the data-service HTTP API, enabling circuit breaker
/// decoration and test mocking.
#[cfg_attr(feature = "test-support", mockall::automock)]
#[async_trait]
pub trait DataServiceClient: Send + Sync {
    /// Fetches all staff members (including sub-group members) for the given group.
    async fn get_resolved_members(
        &self,
        staff_group_id: Uuid,
    ) -> Result<Vec<Staff>, SchedulingServiceError>;
}
