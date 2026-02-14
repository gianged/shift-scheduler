use chrono::{Datelike, NaiveDate};
use std::sync::Arc;
use tracing::Instrument;
use uuid::Uuid;

use shared::types::{JobStatus, ScheduleJob, ShiftAssignment, StaffStatus};

use crate::domain::client::DataServiceClient;
use crate::domain::job::JobRepository;
use crate::domain::scheduler::{SchedulingConfig, gen_schedule};
use crate::error::SchedulingServiceError;

pub struct SchedulingService {
    job_repo: Arc<dyn JobRepository>,
    data_client: Arc<dyn DataServiceClient>,
    config: SchedulingConfig,
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
        }
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

        let job_id = job.id;
        let repo = Arc::clone(&self.job_repo);
        let client = Arc::clone(&self.data_client);
        let config = self.config.clone();

        let span = tracing::info_span!("process_job", %job_id, %staff_group_id);
        tokio::spawn(
            async move {
                if let Err(e) = process_job(
                    job_id,
                    staff_group_id,
                    period_begin_date,
                    repo,
                    client,
                    config,
                )
                .await
                {
                    tracing::error!("Job {job_id} failed: {e}");
                }
            }
            .instrument(span),
        );

        Ok(job)
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
}

#[tracing::instrument(skip(repo, client, config))]
async fn process_job(
    job_id: Uuid,
    staff_group_id: Uuid,
    period_begin_date: NaiveDate,
    repo: Arc<dyn JobRepository>,
    client: Arc<dyn DataServiceClient>,
    config: SchedulingConfig,
) -> Result<(), SchedulingServiceError> {
    tracing::info!("Processing job");
    repo.update_status(job_id, JobStatus::Processing).await?;

    let members = match client.get_resolved_members(staff_group_id).await {
        Ok(m) => m,
        Err(e) => {
            repo.update_status(job_id, JobStatus::Failed).await.ok();
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
            repo.update_status(job_id, JobStatus::Completed).await?;
            tracing::info!("Job completed");
        }
        Err(e) => {
            repo.update_status(job_id, JobStatus::Failed).await.ok();
            tracing::error!("Scheduling failed: {e}");
            return Err(SchedulingServiceError::Internal(format!(
                "Scheduling failed: {e}"
            )));
        }
    }

    Ok(())
}
