use std::sync::Arc;

use async_trait::async_trait;
use shared::types::{Staff, StaffGroup};
use uuid::Uuid;

use super::client::RedisCache;
use crate::domain::membership::MembershipRepository;
use crate::error::DataServiceError;

const TTL: u64 = 300;

fn key_group_members(group_id: Uuid) -> String {
    format!("data-service:membership:group:{group_id}:members")
}

fn key_staff_groups(staff_id: Uuid) -> String {
    format!("data-service:membership:staff:{staff_id}:groups")
}

fn key_resolved(group_id: Uuid) -> String {
    format!("data-service:membership:group:{group_id}:resolved")
}

pub struct CachedMembershipRepository {
    inner: Arc<dyn MembershipRepository>,
    cache: RedisCache,
}

impl CachedMembershipRepository {
    pub fn new(inner: Arc<dyn MembershipRepository>, cache: RedisCache) -> Self {
        Self { inner, cache }
    }

    async fn invalidate_membership(&self, group_id: Uuid, staff_id: Uuid) {
        self.cache
            .delete(&[&key_group_members(group_id), &key_staff_groups(staff_id)])
            .await;
        self.cache
            .delete_by_pattern("data-service:membership:group:*:resolved")
            .await;
    }
}

#[async_trait]
impl MembershipRepository for CachedMembershipRepository {
    async fn get_group_members(&self, group_id: Uuid) -> Result<Vec<Staff>, DataServiceError> {
        let key = key_group_members(group_id);
        if let Some(cached) = self.cache.get::<Vec<Staff>>(&key).await {
            return Ok(cached);
        }
        let output = self.inner.get_group_members(group_id).await?;
        self.cache.set(&key, &output, TTL).await;

        Ok(output)
    }

    async fn get_staff_groups(&self, staff_id: Uuid) -> Result<Vec<StaffGroup>, DataServiceError> {
        let key = key_staff_groups(staff_id);
        if let Some(cached) = self.cache.get::<Vec<StaffGroup>>(&key).await {
            return Ok(cached);
        }
        let output = self.inner.get_staff_groups(staff_id).await?;
        self.cache.set(&key, &output, TTL).await;

        Ok(output)
    }

    async fn resolve_members(&self, group_id: Uuid) -> Result<Vec<Staff>, DataServiceError> {
        let key = key_resolved(group_id);
        if let Some(cached) = self.cache.get::<Vec<Staff>>(&key).await {
            return Ok(cached);
        }
        let output = self.inner.resolve_members(group_id).await?;
        self.cache.set(&key, &output, TTL).await;

        Ok(output)
    }

    async fn add_staff_to_group(
        &self,
        group_id: Uuid,
        staff_id: Uuid,
    ) -> Result<(), DataServiceError> {
        self.inner.add_staff_to_group(group_id, staff_id).await?;
        self.invalidate_membership(group_id, staff_id).await;

        Ok(())
    }

    async fn remove_staff_from_group(
        &self,
        group_id: Uuid,
        staff_id: Uuid,
    ) -> Result<(), DataServiceError> {
        self.inner
            .remove_staff_from_group(group_id, staff_id)
            .await?;
        self.invalidate_membership(group_id, staff_id).await;

        Ok(())
    }
}
