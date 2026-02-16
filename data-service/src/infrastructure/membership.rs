use async_trait::async_trait;
use shared::types::{Staff, StaffGroup};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::membership::{AddMembership, MembershipRepository},
    error::DataServiceError,
};

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
    #[tracing::instrument(skip(self))]
    async fn add_staff_to_group(
        &self,
        group_id: Uuid,
        staff_id: Uuid,
    ) -> Result<(), DataServiceError> {
        let output = sqlx::query!(
            r#"
            INSERT INTO group_memberships (group_id, staff_id) VALUES ($1, $2)
            "#,
            group_id,
            staff_id
        )
        .execute(&self.pool)
        .await;

        match output {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(e)) => {
                let msg = e.message();
                if msg.contains("fk_gm_staff") {
                    Err(DataServiceError::NotFound("Staff not found".to_string()))
                } else if msg.contains("fk_gm_group") {
                    Err(DataServiceError::NotFound("Group not found".to_string()))
                } else if msg.contains("duplicate") || msg.contains("already exists") {
                    Err(DataServiceError::BadRequest(
                        "Staff already in group".to_string(),
                    ))
                } else {
                    Err(sqlx::Error::Database(e).into())
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    #[tracing::instrument(skip(self))]
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

    #[tracing::instrument(skip(self))]
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

    #[tracing::instrument(skip(self))]
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

    #[tracing::instrument(skip(self))]
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

    #[tracing::instrument(skip(self))]
    async fn batch_add_members(
        &self,
        memberships: Vec<AddMembership>,
    ) -> Result<(), DataServiceError> {
        let staff_ids: Vec<Uuid> = memberships.iter().map(|m| m.staff_id).collect();
        let group_ids: Vec<Uuid> = memberships.iter().map(|m| m.group_id).collect();

        sqlx::query!(
            r#"
            INSERT INTO group_memberships (staff_id, group_id)
            SELECT * FROM UNNEST($1::uuid[], $2::uuid[])
            ON CONFLICT DO NOTHING
            "#,
            &staff_ids,
            &group_ids
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
