use async_trait::async_trait;
use chrono::NaiveDate;
use shared::types::{JobStatus, ScheduleJob, ShiftAssignment};
use uuid::Uuid;

use crate::error::SchedulingServiceError;

#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create_job(
        &self,
        group_id: Uuid,
        run_at: NaiveDate,
    ) -> Result<ScheduleJob, SchedulingServiceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScheduleJob>, SchedulingServiceError>;
    async fn update_status(
        &self,
        id: Uuid,
        status: JobStatus,
    ) -> Result<(), SchedulingServiceError>;
    async fn save_assignments(
        &self,
        job_id: Uuid,
        assignments: Vec<ShiftAssignment>,
    ) -> Result<(), SchedulingServiceError>;
    async fn get_assignments(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<ShiftAssignment>, SchedulingServiceError>;
}
