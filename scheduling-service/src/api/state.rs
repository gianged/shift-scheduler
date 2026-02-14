use std::sync::Arc;

use crate::domain::service::SchedulingService;

pub struct SchedulingAppState {
    pub scheduling_service: Arc<SchedulingService>,
}
