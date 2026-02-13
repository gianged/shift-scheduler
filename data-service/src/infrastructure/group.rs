use async_trait::async_trait;
use shared::types::StaffGroup;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::group::{CreateGroup, GroupRepository, UpdateGroup},
    error::DataServiceError,
};

pub struct PgGroupRepository {
    pool: PgPool,
}

impl PgGroupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GroupRepository for PgGroupRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<StaffGroup>, DataServiceError> {
        let output = sqlx::query_as!(
            StaffGroup,
            r#"
            SELECT id, name, parent_group_id, created_at, updated_at
            FROM staff_groups
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(output)
    }

    async fn find_all(&self) -> Result<Vec<StaffGroup>, DataServiceError> {
        let output = sqlx::query_as!(
            StaffGroup,
            r#"
            SELECT id, name, parent_group_id, created_at, updated_at
            FROM staff_groups
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    async fn create(&self, group: CreateGroup) -> Result<StaffGroup, DataServiceError> {
        let output = sqlx::query_as!(
            StaffGroup,
            r#"
            INSERT INTO staff_groups (name, parent_group_id) 
            VALUES ($1, $2) 
            RETURNING id, name, parent_group_id, created_at, updated_at
            "#,
            group.name,
            group.parent_group_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(output)
    }

    async fn batch_create(
        &self,
        groups: Vec<CreateGroup>,
    ) -> Result<Vec<StaffGroup>, DataServiceError> {
        let names: Vec<String> = groups.iter().map(|g| g.name.clone()).collect();
        let parent_ids: Vec<Option<Uuid>> = groups.iter().map(|g| g.parent_group_id).collect();

        let output = sqlx::query_as!(
            StaffGroup,
            r#"
            INSERT INTO staff_groups (name, parent_group_id)
            SELECT * FROM UNNEST($1::varchar[], $2::uuid[])
            RETURNING id, name, parent_group_id, created_at, updated_at
            "#,
            &names,
            &parent_ids as _,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    async fn update(&self, id: Uuid, group: UpdateGroup) -> Result<StaffGroup, DataServiceError> {
        let output = sqlx::query_as!(
            StaffGroup,
            r#" 
            UPDATE staff_groups 
            SET name = COALESCE($2, name), 
                parent_group_id = COALESCE($3, parent_group_id), 
                updated_at = now() 
            WHERE id = $1 
            RETURNING id, name, parent_group_id, created_at, updated_at
            "#,
            id,
            group.name,
            group.parent_group_id as _,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(output)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DataServiceError> {
        let output = sqlx::query!(
            r#"
            DELETE FROM staff_groups
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        if output.rows_affected() == 0 {
            return Err(DataServiceError::NotFound("Group not found".to_string()));
        }

        Ok(())
    }
}
