use chrono::NaiveDate;
use shared::types::{JobStatus, ScheduleJob};
use uuid::Uuid;

/// wrapper for a job in `Pending` status.
/// consuming `start_processing` into to `ProcessingJob`.
pub struct PendingJob {
    inner: ScheduleJob,
}

/// wrapper for a job in `Processing` status.
/// consuming `complete` or `fail` into to terminal states.
pub struct ProcessingJob {
    inner: ScheduleJob,
}

/// Terminal state: job completed successfully.
pub struct CompletedJob {
    inner: ScheduleJob,
}

/// Terminal state: job failed.
pub struct FailedJob {
    inner: ScheduleJob,
}

/// Job is waiting for data service to recover before retrying.
pub struct WaitingForRetryJob {
    inner: ScheduleJob,
}

impl PendingJob {
    pub fn from_schedule_job(job: ScheduleJob) -> Option<Self> {
        if job.status == JobStatus::Pending {
            Some(Self { inner: job })
        } else {
            None
        }
    }

    pub fn id(&self) -> Uuid {
        self.inner.id
    }

    pub fn inner(&self) -> &ScheduleJob {
        &self.inner
    }

    pub fn start_processing(mut self) -> (ProcessingJob, Uuid, JobStatus) {
        let id = self.inner.id;
        self.inner.status = JobStatus::Processing;
        (
            ProcessingJob { inner: self.inner },
            id,
            JobStatus::Processing,
        )
    }
}

impl ProcessingJob {
    pub fn id(&self) -> Uuid {
        self.inner.id
    }

    pub fn staff_group_id(&self) -> Uuid {
        self.inner.staff_group_id
    }

    pub fn period_begin_date(&self) -> NaiveDate {
        self.inner.period_begin_date
    }

    pub fn complete(mut self) -> (CompletedJob, Uuid, JobStatus) {
        let id = self.inner.id;
        self.inner.status = JobStatus::Completed;
        (CompletedJob { inner: self.inner }, id, JobStatus::Completed)
    }

    pub fn fail(mut self) -> (FailedJob, Uuid, JobStatus) {
        let id = self.inner.id;
        self.inner.status = JobStatus::Failed;
        (FailedJob { inner: self.inner }, id, JobStatus::Failed)
    }

    pub fn wait_for_retry(mut self) -> (WaitingForRetryJob, Uuid, JobStatus) {
        let id = self.inner.id;
        self.inner.status = JobStatus::WaitingForRetry;
        (
            WaitingForRetryJob { inner: self.inner },
            id,
            JobStatus::WaitingForRetry,
        )
    }
}

impl CompletedJob {
    pub fn into_inner(self) -> ScheduleJob {
        self.inner
    }
}

impl FailedJob {
    pub fn into_inner(self) -> ScheduleJob {
        self.inner
    }
}

impl WaitingForRetryJob {
    pub fn id(&self) -> Uuid {
        self.inner.id
    }

    pub fn into_pending(mut self) -> (PendingJob, Uuid, JobStatus) {
        let id = self.inner.id;
        self.inner.status = JobStatus::Pending;
        (PendingJob { inner: self.inner }, id, JobStatus::Pending)
    }

    pub fn into_inner(self) -> ScheduleJob {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};

    fn make_job(status: JobStatus) -> ScheduleJob {
        ScheduleJob {
            id: Uuid::new_v4(),
            staff_group_id: Uuid::new_v4(),
            period_begin_date: NaiveDate::from_ymd_opt(2026, 2, 16).unwrap(),
            status,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn pending_from_pending_job_succeeds() {
        let job = make_job(JobStatus::Pending);
        assert!(PendingJob::from_schedule_job(job).is_some());
    }

    #[test]
    fn pending_from_non_pending_job_returns_none() {
        for status in [
            JobStatus::Processing,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::WaitingForRetry,
        ] {
            let job = make_job(status);
            assert!(PendingJob::from_schedule_job(job).is_none());
        }
    }

    #[test]
    fn pending_to_processing_transition() {
        let job = make_job(JobStatus::Pending);
        let job_id = job.id;
        let pending = PendingJob::from_schedule_job(job).unwrap();

        let (processing, id, status) = pending.start_processing();
        assert_eq!(id, job_id);
        assert_eq!(status, JobStatus::Processing);
        assert_eq!(processing.id(), job_id);
    }

    #[test]
    fn processing_to_completed_transition() {
        let job = make_job(JobStatus::Pending);
        let job_id = job.id;
        let pending = PendingJob::from_schedule_job(job).unwrap();
        let (processing, _, _) = pending.start_processing();

        let (completed, id, status) = processing.complete();
        assert_eq!(id, job_id);
        assert_eq!(status, JobStatus::Completed);

        let inner = completed.into_inner();
        assert_eq!(inner.status, JobStatus::Completed);
    }

    #[test]
    fn processing_to_failed_transition() {
        let job = make_job(JobStatus::Pending);
        let job_id = job.id;
        let pending = PendingJob::from_schedule_job(job).unwrap();
        let (processing, _, _) = pending.start_processing();

        let (failed, id, status) = processing.fail();
        assert_eq!(id, job_id);
        assert_eq!(status, JobStatus::Failed);

        let inner = failed.into_inner();
        assert_eq!(inner.status, JobStatus::Failed);
    }

    #[test]
    fn processing_to_waiting_for_retry_transition() {
        let job = make_job(JobStatus::Pending);
        let job_id = job.id;
        let pending = PendingJob::from_schedule_job(job).unwrap();
        let (processing, _, _) = pending.start_processing();

        let (waiting, id, status) = processing.wait_for_retry();
        assert_eq!(id, job_id);
        assert_eq!(status, JobStatus::WaitingForRetry);
        assert_eq!(waiting.id(), job_id);

        let inner = waiting.into_inner();
        assert_eq!(inner.status, JobStatus::WaitingForRetry);
    }

    #[test]
    fn waiting_for_retry_to_pending_transition() {
        let job = make_job(JobStatus::Pending);
        let job_id = job.id;
        let pending = PendingJob::from_schedule_job(job).unwrap();
        let (processing, _, _) = pending.start_processing();
        let (waiting, _, _) = processing.wait_for_retry();

        let (new_pending, id, status) = waiting.into_pending();
        assert_eq!(id, job_id);
        assert_eq!(status, JobStatus::Pending);
        assert_eq!(new_pending.id(), job_id);
    }
}
