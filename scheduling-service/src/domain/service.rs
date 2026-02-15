use chrono::{Datelike, NaiveDate};
use std::sync::Arc;
use tokio_util::task::TaskTracker;
use tracing::Instrument;
use uuid::Uuid;

use shared::types::{JobStatus, ScheduleJob, ShiftAssignment, StaffStatus};

use crate::domain::client::DataServiceClient;
use crate::domain::job::JobRepository;
use crate::domain::job_state::PendingJob;
use crate::domain::scheduler::{SchedulingConfig, gen_schedule};
use crate::error::SchedulingServiceError;

pub struct SchedulingService {
    job_repo: Arc<dyn JobRepository>,
    data_client: Arc<dyn DataServiceClient>,
    config: SchedulingConfig,
    task_tracker: TaskTracker,
}

impl SchedulingService {
    pub fn new(
        job_repo: Arc<dyn JobRepository>,
        data_client: Arc<dyn DataServiceClient>,
        config: SchedulingConfig,
    ) -> Self {
        Self {
            job_repo,
            data_client,
            config,
            task_tracker: TaskTracker::new(),
        }
    }

    pub fn task_tracker(&self) -> &TaskTracker {
        &self.task_tracker
    }

    #[tracing::instrument(skip(self))]
    pub async fn submit_schedule(
        &self,
        staff_group_id: Uuid,
        period_begin_date: NaiveDate,
    ) -> Result<ScheduleJob, SchedulingServiceError> {
        if period_begin_date.weekday() != chrono::Weekday::Mon {
            return Err(SchedulingServiceError::BadRequest(
                "period_begin_date must be a Monday".to_string(),
            ));
        }

        let job = self
            .job_repo
            .create_job(staff_group_id, period_begin_date)
            .await?;

        let pending_job = PendingJob::from_schedule_job(job.clone()).ok_or_else(|| {
            SchedulingServiceError::Internal(format!(
                "Newly created job {} has unexpected status {:?}",
                job.id, job.status
            ))
        })?;

        self.spawn_process_job(pending_job);

        Ok(job)
    }

    pub fn spawn_process_job(&self, pending_job: PendingJob) {
        let job_id = pending_job.id();
        let staff_group_id = pending_job.inner().staff_group_id;
        let repo = Arc::clone(&self.job_repo);
        let client = Arc::clone(&self.data_client);
        let config = self.config.clone();

        let span = tracing::info_span!("process_job", %job_id, %staff_group_id);
        self.task_tracker.spawn(
            async move {
                if let Err(e) = process_job(pending_job, repo, client, config).await {
                    tracing::error!("Job {job_id} failed: {e}");
                }
            }
            .instrument(span),
        );
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_status(&self, job_id: Uuid) -> Result<ScheduleJob, SchedulingServiceError> {
        self.job_repo.find_by_id(job_id).await?.ok_or_else(|| {
            SchedulingServiceError::NotFound(format!("Schedule job {job_id} not found"))
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_result(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<ShiftAssignment>, SchedulingServiceError> {
        let job = self.get_status(job_id).await?;

        if job.status != JobStatus::Completed {
            return Err(SchedulingServiceError::BadRequest(format!(
                "Job is not completed, current status: {:?}",
                job.status
            )));
        }

        self.job_repo.get_assignments(job_id).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn recover_stale_jobs(&self) -> Result<(), SchedulingServiceError> {
        let stale_jobs = self.job_repo.find_by_status(JobStatus::Processing).await?;

        if stale_jobs.is_empty() {
            tracing::info!("No stale jobs to recover");
            return Ok(());
        }

        tracing::info!(count = stale_jobs.len(), "Recovering stale jobs");

        for job in stale_jobs {
            let job_id = job.id;
            tracing::info!(%job_id, "Recovering stale job");

            self.job_repo.delete_assignments(job_id).await?;
            self.job_repo
                .update_status(job_id, JobStatus::Pending)
                .await?;

            let refreshed = self.job_repo.find_by_id(job_id).await?;
            if let Some(job) = refreshed {
                if let Some(pending) = PendingJob::from_schedule_job(job) {
                    self.spawn_process_job(pending);
                } else {
                    tracing::warn!(%job_id, "Job no longer in Pending status after reset");
                }
            }
        }

        Ok(())
    }
}

#[tracing::instrument(skip(pending_job, repo, client, config), fields(job_id = %pending_job.id()))]
async fn process_job(
    pending_job: PendingJob,
    repo: Arc<dyn JobRepository>,
    client: Arc<dyn DataServiceClient>,
    config: SchedulingConfig,
) -> Result<(), SchedulingServiceError> {
    tracing::info!("Processing job");

    let (processing_job, job_id, status) = pending_job.start_processing();
    repo.update_status(job_id, status).await?;

    let staff_group_id = processing_job.staff_group_id();
    let period_begin_date = processing_job.period_begin_date();

    let members = match client.get_resolved_members(staff_group_id).await {
        Ok(m) => m,
        Err(e) => {
            let (_failed, id, status) = processing_job.fail();
            repo.update_status(id, status).await.ok();
            return Err(e);
        }
    };

    let active_ids: Vec<_> = members
        .into_iter()
        .filter(|s| s.status == StaffStatus::Active)
        .map(|s| s.id)
        .collect();

    match gen_schedule(&active_ids, period_begin_date, &config) {
        Ok(assignments) => {
            repo.save_assignments(job_id, assignments).await?;
            let (_completed, id, status) = processing_job.complete();
            repo.update_status(id, status).await?;
            tracing::info!("Job completed");
        }
        Err(e) => {
            let (_failed, id, status) = processing_job.fail();
            repo.update_status(id, status).await.ok();
            tracing::error!("Scheduling failed: {e}");
            return Err(SchedulingServiceError::Internal(format!(
                "Scheduling failed: {e}"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::client::MockDataServiceClient;
    use crate::domain::job::MockJobRepository;
    use crate::domain::scheduler::SchedulingConfig;

    fn make_service(
        job_repo: MockJobRepository,
        data_client: MockDataServiceClient,
    ) -> SchedulingService {
        SchedulingService::new(
            Arc::new(job_repo),
            Arc::new(data_client),
            SchedulingConfig::default(),
        )
    }

    #[tokio::test]
    async fn submit_schedule_rejects_non_monday() {
        let repo = MockJobRepository::new();
        let client = MockDataServiceClient::new();
        let svc = make_service(repo, client);

        // 2026-02-17 is Tuesday
        let tuesday = NaiveDate::from_ymd_opt(2026, 2, 17).unwrap();
        let output = svc.submit_schedule(Uuid::new_v4(), tuesday).await;

        assert!(output.is_err());
        assert!(matches!(
            output.unwrap_err(),
            SchedulingServiceError::BadRequest(_)
        ));
    }

    #[tokio::test]
    async fn get_status_not_found() {
        let mut repo = MockJobRepository::new();
        repo.expect_find_by_id().returning(|_| Ok(None));

        let client = MockDataServiceClient::new();
        let svc = make_service(repo, client);

        let output = svc.get_status(Uuid::new_v4()).await;

        assert!(output.is_err());
        assert!(matches!(
            output.unwrap_err(),
            SchedulingServiceError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn get_result_not_completed() {
        let mut repo = MockJobRepository::new();
        let job_id = Uuid::new_v4();
        let job = ScheduleJob {
            id: job_id,
            staff_group_id: Uuid::new_v4(),
            period_begin_date: NaiveDate::from_ymd_opt(2026, 2, 16).unwrap(),
            status: JobStatus::Processing,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(job.clone())));

        let client = MockDataServiceClient::new();
        let svc = make_service(repo, client);

        let output = svc.get_result(job_id).await;

        assert!(output.is_err());
        assert!(matches!(
            output.unwrap_err(),
            SchedulingServiceError::BadRequest(_)
        ));
    }
}
