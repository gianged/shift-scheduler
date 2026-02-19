use async_trait::async_trait;
use chrono::NaiveDate;
use shared::types::{JobStatus, ScheduleJob, ShiftAssignment, ShiftType};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::job::{JobRepository, NewShiftAssignment},
    error::SchedulingServiceError,
};

pub struct PgJobRepository {
    pool: PgPool,
}

impl PgJobRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl JobRepository for PgJobRepository {
    #[tracing::instrument(skip(self))]
    async fn create_job(
        &self,
        staff_group_id: Uuid,
        period_begin_date: NaiveDate,
    ) -> Result<ScheduleJob, SchedulingServiceError> {
        let output = sqlx::query_as!(ScheduleJob,
            r#"
            INSERT INTO schedule_jobs (staff_group_id, period_begin_date)
            VALUES ($1, $2)
            RETURNING id, staff_group_id, period_begin_date, status AS "status: _", created_at, updated_at
            "#,
            staff_group_id,
            period_begin_date
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self))]
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScheduleJob>, SchedulingServiceError> {
        let output = sqlx::query_as!(
            ScheduleJob,
            r#"
            SELECT id, staff_group_id, period_begin_date, status AS "status: _", created_at, updated_at
            FROM schedule_jobs
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self))]
    async fn update_status(
        &self,
        id: Uuid,
        status: JobStatus,
    ) -> Result<(), SchedulingServiceError> {
        let output = sqlx::query!(
            r#"
            UPDATE schedule_jobs
            SET status = $2, updated_at = now()
            WHERE id = $1
            "#,
            id,
            status as _,
        )
        .execute(&self.pool)
        .await?;

        if output.rows_affected() == 0 {
            return Err(SchedulingServiceError::NotFound(format!(
                "Schedule job {id} not found"
            )));
        }

        Ok(())
    }

    #[tracing::instrument(skip(self, assignments))]
    async fn save_assignments(
        &self,
        job_id: Uuid,
        assignments: Vec<NewShiftAssignment>,
    ) -> Result<(), SchedulingServiceError> {
        let job_ids: Vec<Uuid> = vec![job_id; assignments.len()];
        let staff_ids: Vec<Uuid> = assignments.iter().map(|a| a.staff_id).collect();
        let dates: Vec<NaiveDate> = assignments.iter().map(|a| a.date).collect();
        let shift_types: Vec<ShiftType> = assignments.iter().map(|a| a.shift_type).collect();

        sqlx::query(
            r#"
            INSERT INTO shift_assignments (job_id, staff_id, date, shift_type)
            SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::date[], $4::shift_type[])
            "#,
        )
        .bind(&job_ids)
        .bind(&staff_ids)
        .bind(&dates)
        .bind(&shift_types)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get_assignments(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<ShiftAssignment>, SchedulingServiceError> {
        let output = sqlx::query_as!(
            ShiftAssignment,
            r#"
            SELECT id, job_id, staff_id, date, shift_type AS "shift_type: _"
            FROM shift_assignments
            WHERE job_id = $1
            ORDER BY staff_id, date
            "#,
            job_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self))]
    async fn find_by_status(
        &self,
        status: JobStatus,
    ) -> Result<Vec<ScheduleJob>, SchedulingServiceError> {
        let output = sqlx::query_as!(
            ScheduleJob,
            r#"
            SELECT id, staff_group_id, period_begin_date, status AS "status: _", created_at, updated_at
            FROM schedule_jobs
            WHERE status = $1
            ORDER BY created_at ASC
            "#,
            status as _,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_assignments(&self, job_id: Uuid) -> Result<(), SchedulingServiceError> {
        sqlx::query!(
            r#"
            DELETE FROM shift_assignments
            WHERE job_id = $1
            "#,
            job_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
