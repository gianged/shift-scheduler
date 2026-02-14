use async_trait::async_trait;
use shared::types::Staff;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::staff::{CreateStaff, StaffRepository, UpdateStaff},
    error::DataServiceError,
};

pub struct PgStaffRepository {
    pool: PgPool,
}

impl PgStaffRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StaffRepository for PgStaffRepository {
    #[tracing::instrument(skip(self))]
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Staff>, DataServiceError> {
        let output = sqlx::query_as!(
            Staff,
            r#"
            SELECT id, name, email, position, status AS "status: _", created_at, updated_at
            FROM staff
            WHERE id = $1
        "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self))]
    async fn find_all(&self) -> Result<Vec<Staff>, DataServiceError> {
        let output = sqlx::query_as!(
            Staff,
            r#"
            SELECT id, name, email, position, status AS "status: _", created_at, updated_at
            FROM staff
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self))]
    async fn create(&self, staff: CreateStaff) -> Result<Staff, DataServiceError> {
        let output = sqlx::query_as!(
            Staff,
            r#"
            INSERT INTO staff (name, email, position)
            VALUES ($1, $2, $3)
            RETURNING id, name, email, position, status AS "status: _", created_at, updated_at
            "#,
            staff.name,
            staff.email,
            staff.position
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self, staffs))]
    async fn batch_create(&self, staffs: Vec<CreateStaff>) -> Result<Vec<Staff>, DataServiceError> {
        let names: Vec<String> = staffs.iter().map(|s| s.name.clone()).collect();
        let emails: Vec<String> = staffs.iter().map(|s| s.email.clone()).collect();
        let positions: Vec<String> = staffs.iter().map(|s| s.position.clone()).collect();

        let output = sqlx::query_as!(
            Staff,
            r#"
                INSERT INTO staff(name, email, position)
                SELECT * FROM UNNEST($1::varchar[], $2::varchar[], $3::varchar[])
                RETURNING id, name, email, position, status AS "status: _", created_at, updated_at
            "#,
            &names,
            &emails,
            &positions
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    #[tracing::instrument(skip(self))]
    async fn update(&self, id: Uuid, staff: UpdateStaff) -> Result<Staff, DataServiceError> {
        let output = sqlx::query_as!(
            Staff,
            r#"
            UPDATE staff
            SET name = COALESCE($2, name),
                email = COALESCE($3,email),
                position = COALESCE($4, position),
                status = COALESCE($5, status),
                updated_at = now()
            WHERE id = $1
            RETURNING id, name, email, position, status AS "status: _", created_at, updated_at
            "#,
            id,
            staff.name,
            staff.email,
            staff.position,
            staff.status as _,
        )
        .fetch_optional(&self.pool)
        .await?;

        output.ok_or_else(|| DataServiceError::NotFound("Staff not found".to_string()))
    }

    #[tracing::instrument(skip(self))]
    async fn deactivate(&self, id: Uuid) -> Result<(), DataServiceError> {
        let output = sqlx::query!(
            r#"
            UPDATE staff
            SET status = 'INACTIVE', updated_at = now()
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        if output.rows_affected() == 0 {
            return Err(DataServiceError::NotFound("Staff not found".to_string()));
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&self, id: Uuid) -> Result<(), DataServiceError> {
        let output = sqlx::query!(
            r#"
            DELETE FROM staff
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        if output.rows_affected() == 0 {
            return Err(DataServiceError::NotFound("Staff not found".to_string()));
        }

        Ok(())
    }
}
