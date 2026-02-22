use async_trait::async_trait;
use chrono::NaiveDate;
use shared::types::{JobStatus, ScheduleJob, ShiftAssignment, ShiftType};
use uuid::Uuid;

use crate::error::SchedulingServiceError;

/// A shift assignment to be persisted, before it has a database-generated ID.
#[derive(Debug)]
pub struct NewShiftAssignment {
    pub staff_id: Uuid,
    pub date: NaiveDate,
    pub shift_type: ShiftType,
}

/// Persistence operations for schedule jobs and their shift assignments.
#[cfg_attr(feature = "test-support", mockall::automock)]
#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create_job(
        &self,
        staff_group_id: Uuid,
        period_begin_date: NaiveDate,
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
        assignments: Vec<NewShiftAssignment>,
    ) -> Result<(), SchedulingServiceError>;
    async fn get_assignments(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<ShiftAssignment>, SchedulingServiceError>;
    async fn find_by_status(
        &self,
        status: JobStatus,
    ) -> Result<Vec<ScheduleJob>, SchedulingServiceError>;
    async fn delete_assignments(&self, job_id: Uuid) -> Result<(), SchedulingServiceError>;
}
