use chrono::{Datelike, NaiveDate};
use std::sync::Arc;
use tokio_util::task::TaskTracker;
use tracing::Instrument;
use uuid::Uuid;

use shared::types::{JobStatus, ScheduleJob, ScheduleResult, StaffStatus};

use crate::domain::client::DataServiceClient;
use crate::domain::job::JobRepository;
use crate::domain::job_state::PendingJob;
use crate::domain::scheduler::{SchedulingConfig, SchedulingRule, gen_schedule};
use crate::error::SchedulingServiceError;

/// Orchestrates schedule job lifecycle: submission, processing, status queries,
/// and recovery of stale or retry-waiting jobs.
pub struct SchedulingService {
    job_repo: Arc<dyn JobRepository>,
    data_client: Arc<dyn DataServiceClient>,
    config: SchedulingConfig,
    rules: Arc<Vec<Box<dyn SchedulingRule>>>,
    task_tracker: TaskTracker,
}

impl SchedulingService {
    /// Creates a new scheduling service with the given repository, data client, and config.
    pub fn new(
        job_repo: Arc<dyn JobRepository>,
        data_client: Arc<dyn DataServiceClient>,
        config: SchedulingConfig,
    ) -> Self {
        let rules = Arc::new(config.build_rules());
        Self {
            job_repo,
            data_client,
            config,
            rules,
            task_tracker: TaskTracker::new(),
        }
    }

    pub fn task_tracker(&self) -> &TaskTracker {
        &self.task_tracker
    }

    /// Validates the request, creates a new schedule job in the database, and spawns
    /// asynchronous processing. Returns the created job immediately (fire-and-forget).
    #[tracing::instrument(skip(self))]
    pub async fn submit_schedule(
        &self,
        staff_group_id: Uuid,
        period_begin_date: NaiveDate,
    ) -> Result<ScheduleJob, SchedulingServiceError> {
        if period_begin_date.weekday() != chrono::Weekday::Mon {
            return Err(SchedulingServiceError::BadRequest(
                "period_begin_date must be a Monday".into(),
            ));
        }

        let today = shared::time::today_in(self.config.timezone());
        if period_begin_date < today {
            return Err(SchedulingServiceError::BadRequest(
                "period_begin_date must not be in the past".into(),
            ));
        }

        let job = self
            .job_repo
            .create_job(staff_group_id, period_begin_date)
            .await?;

        let pending_job = PendingJob::from_schedule_job(job.clone()).ok_or_else(|| {
            let job_id = job.id;
            let job_status = job.status;
            SchedulingServiceError::Internal(format!(
                "Newly created job {job_id} has unexpected status {job_status:?}"
            ))
        })?;

        self.spawn_process_job(pending_job);

        Ok(job)
    }

    /// Spawns a background task to process the given pending job.
    pub fn spawn_process_job(&self, pending_job: PendingJob) {
        let job_id = pending_job.id();
        let staff_group_id = pending_job.inner().staff_group_id;
        let repo = Arc::clone(&self.job_repo);
        let client = Arc::clone(&self.data_client);
        let rules = Arc::clone(&self.rules);

        let span = tracing::info_span!("process_job", %job_id, %staff_group_id);
        self.task_tracker.spawn(
            async move {
                if let Err(e) = process_job(pending_job, repo, client, rules).await {
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
    pub async fn get_result(&self, job_id: Uuid) -> Result<ScheduleResult, SchedulingServiceError> {
        let job = self.get_status(job_id).await?;

        if job.status != JobStatus::Completed {
            let status = job.status;
            return Err(SchedulingServiceError::BadRequest(format!(
                "Job is not completed, current status: {status:?}"
            )));
        }

        let assignments = self.job_repo.get_assignments(job_id).await?;

        Ok(ScheduleResult {
            schedule_id: job.id,
            period_begin_date: job.period_begin_date,
            staff_group_id: job.staff_group_id,
            assignments,
        })
    }

    /// Recovers jobs left in `Processing` state (e.g., from a previous crash) by resetting
    /// them to `Pending` and re-spawning processing. Also retries any `WaitingForRetry` jobs.
    #[tracing::instrument(skip(self))]
    pub async fn recover_stale_jobs(&self) -> Result<(), SchedulingServiceError> {
        let stale_jobs = self.job_repo.find_by_status(JobStatus::Processing).await?;

        if stale_jobs.is_empty() {
            tracing::info!("No stale jobs to recover");
        } else {
            tracing::info!(count = stale_jobs.len(), "Recovering stale jobs");
            for job in stale_jobs {
                let job_id = job.id;
                tracing::info!(%job_id, "Recovering stale job");

                self.job_repo.delete_assignments(job_id).await?;
                self.job_repo
                    .update_status(job_id, JobStatus::Pending)
                    .await?;

                self.respawn_as_pending(job_id).await?;
            }
        }

        // Also recover jobs waiting for retry from a previous run
        self.retry_waiting_jobs().await?;

        Ok(())
    }

    /// Resets all `WaitingForRetry` jobs to `Pending` and re-spawns processing.
    /// Called by the health check when the data service recovers.
    #[tracing::instrument(skip(self))]
    pub async fn retry_waiting_jobs(&self) -> Result<(), SchedulingServiceError> {
        let waiting_jobs = self
            .job_repo
            .find_by_status(JobStatus::WaitingForRetry)
            .await?;

        if waiting_jobs.is_empty() {
            tracing::info!("No waiting-for-retry jobs to retry");
            return Ok(());
        }

        tracing::info!(
            count = waiting_jobs.len(),
            "Retrying waiting-for-retry jobs"
        );

        for job in waiting_jobs {
            let job_id = job.id;
            tracing::info!(%job_id, "Retrying waiting job");

            self.job_repo
                .update_status(job_id, JobStatus::Pending)
                .await?;

            self.respawn_as_pending(job_id).await?;
        }

        Ok(())
    }

    /// Reloads a job from the database and spawns processing if it is in `Pending` status.
    async fn respawn_as_pending(&self, job_id: Uuid) -> Result<(), SchedulingServiceError> {
        let Some(job) = self.job_repo.find_by_id(job_id).await? else {
            return Ok(());
        };
        if let Some(pending) = PendingJob::from_schedule_job(job) {
            self.spawn_process_job(pending);
        } else {
            tracing::warn!(%job_id, "Job no longer in Pending status after reset");
        }
        Ok(())
    }
}

/// Processes a single schedule job: transitions to `Processing`, fetches staff from the
/// data service, generates the schedule, and saves assignments.
///
/// On data-service connectivity errors (`CircuitOpen`, `DataServiceUnavailable`), the job
/// is moved to `WaitingForRetry` instead of `Failed`.
#[tracing::instrument(skip(pending_job, repo, client, rules), fields(job_id = %pending_job.id()))]
async fn process_job(
    pending_job: PendingJob,
    repo: Arc<dyn JobRepository>,
    client: Arc<dyn DataServiceClient>,
    rules: Arc<Vec<Box<dyn SchedulingRule>>>,
) -> Result<(), SchedulingServiceError> {
    tracing::info!("Processing job");

    let (processing_job, job_id, status) = pending_job.start_processing();
    repo.update_status(job_id, status).await?;

    let staff_group_id = processing_job.staff_group_id();
    let period_begin_date = processing_job.period_begin_date();

    let members = match client.get_resolved_members(staff_group_id).await {
        Ok(m) => m,
        Err(
            e @ (SchedulingServiceError::CircuitOpen
            | SchedulingServiceError::DataServiceUnavailable(_)),
        ) => {
            let (_waiting, id, status) = processing_job.wait_for_retry();
            if let Err(e) = repo.update_status(id, status).await {
                tracing::error!(%id, "Failed to mark job as waiting for retry: {e}");
            }
            tracing::warn!(%id, "Job marked as waiting for retry due to data service unavailability");
            return Err(e);
        }
        Err(e) => {
            let (_failed, id, status) = processing_job.fail();
            if let Err(e) = repo.update_status(id, status).await {
                tracing::error!(%id, "Failed to mark job as failed: {e}");
            }
            return Err(e);
        }
    };

    let active_ids: Vec<_> = members
        .into_iter()
        .filter(|s| s.status == StaffStatus::Active)
        .map(|s| s.id)
        .collect();

    match gen_schedule(&active_ids, period_begin_date, &rules) {
        Ok(assignments) => {
            repo.save_assignments(job_id, assignments).await?;
            let (_completed, id, status) = processing_job.complete();
            repo.update_status(id, status).await?;
            tracing::info!("Job completed");
        }
        Err(e) => {
            let (_failed, id, status) = processing_job.fail();
            if let Err(update_err) = repo.update_status(id, status).await {
                tracing::error!(%id, "Failed to mark job as failed: {update_err}");
            }
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
    use crate::domain::job::{MockJobRepository, NewShiftAssignment};
    use crate::domain::scheduler::SchedulingConfig;
    use shared::types::ShiftAssignment;
    use std::sync::Mutex;

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

    fn make_job(status: JobStatus) -> ScheduleJob {
        ScheduleJob {
            id: Uuid::new_v4(),
            staff_group_id: Uuid::new_v4(),
            period_begin_date: NaiveDate::from_ymd_opt(2026, 2, 16).unwrap(),
            status,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
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
        let job = make_job(JobStatus::Processing);
        let job_id = job.id;
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

    #[tokio::test]
    async fn get_result_returns_schedule_result_with_metadata() {
        let mut repo = MockJobRepository::new();
        let job = make_job(JobStatus::Completed);
        let job_id = job.id;
        let staff_group_id = job.staff_group_id;
        let period_begin_date = job.period_begin_date;

        repo.expect_find_by_id()
            .returning(move |_| Ok(Some(job.clone())));

        let assignment = ShiftAssignment {
            id: Uuid::new_v4(),
            job_id,
            staff_id: Uuid::new_v4(),
            date: period_begin_date,
            shift_type: shared::types::ShiftType::Morning,
        };
        let assignments = vec![assignment.clone()];
        repo.expect_get_assignments()
            .returning(move |_| Ok(assignments.clone()));

        let client = MockDataServiceClient::new();
        let svc = make_service(repo, client);

        let output = svc.get_result(job_id).await.unwrap();

        assert_eq!(output.schedule_id, job_id);
        assert_eq!(output.staff_group_id, staff_group_id);
        assert_eq!(output.period_begin_date, period_begin_date);
        assert_eq!(output.assignments.len(), 1);
        assert_eq!(output.assignments[0].id, assignment.id);
    }

    #[tokio::test]
    async fn process_job_happy_path() {
        let job = make_job(JobStatus::Pending);
        let pending = PendingJob::from_schedule_job(job).unwrap();

        let mut repo = MockJobRepository::new();

        // Track status transitions
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();
        repo.expect_update_status().returning(move |_, status| {
            statuses_clone.lock().unwrap().push(status);
            Ok(())
        });

        // Capture saved assignments
        let saved = Arc::new(Mutex::new(Vec::<NewShiftAssignment>::new()));
        let saved_clone = saved.clone();
        repo.expect_save_assignments()
            .returning(move |_, assignments| {
                *saved_clone.lock().unwrap() = assignments;
                Ok(())
            });

        let mut client = MockDataServiceClient::new();
        let staff_ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let staff: Vec<_> = staff_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| shared::types::Staff {
                id,
                name: format!("Staff {i}"),
                email: format!("s{i}@example.com"),
                position: "Nurse".to_string(),
                status: StaffStatus::Active,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .collect();
        client
            .expect_get_resolved_members()
            .returning(move |_| Ok(staff.clone()));

        let rules = Arc::new(SchedulingConfig::default().build_rules());

        let output = process_job(pending, Arc::new(repo), Arc::new(client), rules).await;
        assert!(output.is_ok());

        // Verify status transitions: Pending -> Processing -> Completed
        let recorded = statuses.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], JobStatus::Processing);
        assert_eq!(recorded[1], JobStatus::Completed);

        // Verify assignments were saved (4 staff * 28 days = 112)
        let assignments = saved.lock().unwrap();
        assert_eq!(assignments.len(), 4 * 28);

        // Verify all staff have assignments
        for &sid in &staff_ids {
            let count = assignments.iter().filter(|a| a.staff_id == sid).count();
            assert_eq!(count, 28, "Staff {sid} should have 28 assignments");
        }
    }

    #[tokio::test]
    async fn process_job_data_service_error_marks_failed() {
        let job = make_job(JobStatus::Pending);
        let pending = PendingJob::from_schedule_job(job).unwrap();

        let mut repo = MockJobRepository::new();

        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();
        repo.expect_update_status().returning(move |_, status| {
            statuses_clone.lock().unwrap().push(status);
            Ok(())
        });

        let mut client = MockDataServiceClient::new();
        client.expect_get_resolved_members().returning(|_| {
            Err(SchedulingServiceError::DataService(
                "Connection refused".into(),
            ))
        });

        let rules = Arc::new(SchedulingConfig::default().build_rules());

        let output = process_job(pending, Arc::new(repo), Arc::new(client), rules).await;
        assert!(output.is_err());

        // Verify status transitions: Pending -> Processing -> Failed
        let recorded = statuses.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], JobStatus::Processing);
        assert_eq!(recorded[1], JobStatus::Failed);
    }

    #[tokio::test]
    async fn process_job_filters_inactive_staff() {
        let job = make_job(JobStatus::Pending);
        let pending = PendingJob::from_schedule_job(job).unwrap();

        let mut repo = MockJobRepository::new();
        repo.expect_update_status().returning(|_, _| Ok(()));

        let saved = Arc::new(Mutex::new(Vec::<NewShiftAssignment>::new()));
        let saved_clone = saved.clone();
        repo.expect_save_assignments()
            .returning(move |_, assignments| {
                *saved_clone.lock().unwrap() = assignments;
                Ok(())
            });

        let active_id = Uuid::new_v4();
        let inactive_id = Uuid::new_v4();
        let mut client = MockDataServiceClient::new();
        let staff = vec![
            shared::types::Staff {
                id: active_id,
                name: "Active".to_string(),
                email: "a@example.com".to_string(),
                position: "Nurse".to_string(),
                status: StaffStatus::Active,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            shared::types::Staff {
                id: inactive_id,
                name: "Inactive".to_string(),
                email: "i@example.com".to_string(),
                position: "Nurse".to_string(),
                status: StaffStatus::Inactive,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        ];
        client
            .expect_get_resolved_members()
            .returning(move |_| Ok(staff.clone()));

        let rules = Arc::new(SchedulingConfig::default().build_rules());

        let output = process_job(pending, Arc::new(repo), Arc::new(client), rules).await;
        assert!(output.is_ok());

        // Only the active staff member should have assignments
        let assignments = saved.lock().unwrap();
        assert_eq!(assignments.len(), 28);
        assert!(assignments.iter().all(|a| a.staff_id == active_id));
    }

    #[tokio::test]
    async fn process_job_circuit_open_marks_waiting_for_retry() {
        let job = make_job(JobStatus::Pending);
        let pending = PendingJob::from_schedule_job(job).unwrap();

        let mut repo = MockJobRepository::new();

        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();
        repo.expect_update_status().returning(move |_, status| {
            statuses_clone.lock().unwrap().push(status);
            Ok(())
        });

        let mut client = MockDataServiceClient::new();
        client
            .expect_get_resolved_members()
            .returning(|_| Err(SchedulingServiceError::CircuitOpen));

        let rules = Arc::new(SchedulingConfig::default().build_rules());

        let output = process_job(pending, Arc::new(repo), Arc::new(client), rules).await;
        assert!(output.is_err());
        assert!(matches!(
            output.unwrap_err(),
            SchedulingServiceError::CircuitOpen
        ));

        // Verify status transitions: Pending -> Processing -> WaitingForRetry
        let recorded = statuses.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], JobStatus::Processing);
        assert_eq!(recorded[1], JobStatus::WaitingForRetry);
    }

    #[tokio::test]
    async fn process_job_data_service_unavailable_marks_waiting_for_retry() {
        let job = make_job(JobStatus::Pending);
        let pending = PendingJob::from_schedule_job(job).unwrap();

        let mut repo = MockJobRepository::new();

        let statuses = Arc::new(Mutex::new(Vec::new()));
        let statuses_clone = statuses.clone();
        repo.expect_update_status().returning(move |_, status| {
            statuses_clone.lock().unwrap().push(status);
            Ok(())
        });

        let mut client = MockDataServiceClient::new();
        client.expect_get_resolved_members().returning(|_| {
            Err(SchedulingServiceError::DataServiceUnavailable(
                "connection refused".into(),
            ))
        });

        let rules = Arc::new(SchedulingConfig::default().build_rules());

        let output = process_job(pending, Arc::new(repo), Arc::new(client), rules).await;
        assert!(output.is_err());

        // Verify status transitions: Pending -> Processing -> WaitingForRetry
        let recorded = statuses.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], JobStatus::Processing);
        assert_eq!(recorded[1], JobStatus::WaitingForRetry);
    }
}
