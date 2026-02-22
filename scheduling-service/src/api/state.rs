use std::sync::Arc;

use crate::domain::service::SchedulingService;

/// Shared application state for the scheduling service axum router.
pub struct SchedulingAppState {
    pub scheduling_service: Arc<SchedulingService>,
}
