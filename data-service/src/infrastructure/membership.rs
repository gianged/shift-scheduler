use async_trait::async_trait;
use shared::types::{Staff, StaffGroup};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{domain::membership::MembershipRepository, error::DataServiceError};

pub struct PgMembershipRepository {
    pool: PgPool,
}

impl PgMembershipRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MembershipRepository for PgMembershipRepository {
    async fn add_staff_to_group(
        &self,
        group_id: Uuid,
        staff_id: Uuid,
    ) -> Result<(), DataServiceError> {
        sqlx::query!(
            r#"
            INSERT INTO group_memberships (group_id, staff_id) VALUES ($1, $2)
            "#,
            group_id,
            staff_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn remove_staff_from_group(
        &self,
        group_id: Uuid,
        staff_id: Uuid,
    ) -> Result<(), DataServiceError> {
        let output = sqlx::query!(
            r#"
            DELETE FROM group_memberships
            WHERE group_id = $1 AND staff_id = $2
            "#,
            group_id,
            staff_id,
        )
        .execute(&self.pool)
        .await?;

        if output.rows_affected() == 0 {
            return Err(DataServiceError::NotFound(
                "Membership not found".to_string(),
            ));
        }

        Ok(())
    }

    async fn get_group_members(&self, group_id: Uuid) -> Result<Vec<Staff>, DataServiceError> {
        let output = sqlx::query_as!(
            Staff,
            r#"
            SELECT s.id, s.name, s.email, s.position, s.status as "status: _", s.created_at, s.updated_at
            FROM staff s
            JOIN group_memberships gm ON s.id = gm.staff_id
            WHERE gm.group_id = $1
            "#,
            group_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    async fn get_staff_groups(&self, staff_id: Uuid) -> Result<Vec<StaffGroup>, DataServiceError> {
        let output = sqlx::query_as!(
            StaffGroup,
            r#"
            SELECT sg.id, sg.name, sg.parent_group_id, sg.created_at, sg.updated_at
            FROM staff_groups sg
            JOIN group_memberships gm ON sg.id = gm.group_id
            WHERE gm.staff_id = $1
            "#,
            staff_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }

    async fn resolve_members(&self, group_id: Uuid) -> Result<Vec<Staff>, DataServiceError> {
        let output = sqlx::query_as!(
            Staff,
            r#"
            WITH RECURSIVE group_tree AS (
                SELECT id FROM staff_groups WHERE id = $1
                UNION ALL
                SELECT sg.id FROM staff_groups sg
                JOIN group_tree gt ON sg.parent_group_id = gt.id
            )
            SELECT DISTINCT s.id, s.name, s.email, s.position, s.status as "status: _", s.created_at, s.updated_at
            FROM staff s
            JOIN group_memberships gm ON s.id = gm.staff_id
            JOIN group_tree gt ON gm.group_id = gt.id
            "#,
            group_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(output)
    }
}
