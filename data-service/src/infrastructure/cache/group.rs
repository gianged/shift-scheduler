use std::sync::Arc;

use async_trait::async_trait;
use shared::types::StaffGroup;
use uuid::Uuid;

use super::client::RedisCache;
use crate::domain::group::{CreateGroup, GroupRepository, UpdateGroup};
use crate::error::DataServiceError;

/// Cache key for the full group list.
const KEY_ALL: &str = "data-service:groups:all";
/// TTL in seconds for the full group list cache entry.
const TTL_ALL: u64 = 300;
/// TTL in seconds for individual group-by-id cache entries.
const TTL_BY_ID: u64 = 600;

fn key_by_id(id: Uuid) -> String {
    format!("data-service:groups:id:{id}")
}

/// Cache-aside decorator around a [`GroupRepository`].
///
/// Reads check Redis first; writes delegate to the inner repository and
/// invalidate relevant cache keys.
pub struct CachedGroupRepository {
    inner: Arc<dyn GroupRepository>,
    cache: RedisCache,
}

impl CachedGroupRepository {
    pub fn new(inner: Arc<dyn GroupRepository>, cache: RedisCache) -> Self {
        Self { inner, cache }
    }

    async fn invalidate_lists(&self) {
        self.cache.delete(&[KEY_ALL]).await;
    }

    async fn invalidate_with_membership(&self, id: Uuid) {
        self.cache.delete(&[KEY_ALL, &key_by_id(id)]).await;
        self.cache
            .delete_by_pattern("data-service:membership:staff:*:groups")
            .await;
        self.cache
            .delete_by_pattern("data-service:membership:group:*:resolved")
            .await;
    }

    async fn invalidate_all(&self, id: Uuid) {
        self.cache.delete(&[KEY_ALL, &key_by_id(id)]).await;
        self.cache
            .delete_by_pattern("data-service:membership:*")
            .await;
    }
}

#[async_trait]
impl GroupRepository for CachedGroupRepository {
    async fn find_all(&self) -> Result<Vec<StaffGroup>, DataServiceError> {
        if let Some(cached) = self.cache.get::<Vec<StaffGroup>>(KEY_ALL).await {
            return Ok(cached);
        }
        let output = self.inner.find_all().await?;
        self.cache.set(KEY_ALL, &output, TTL_ALL).await;

        Ok(output)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<StaffGroup>, DataServiceError> {
        let key = key_by_id(id);
        if let Some(cached) = self.cache.get::<Option<StaffGroup>>(&key).await {
            return Ok(cached);
        }
        let output = self.inner.find_by_id(id).await?;
        self.cache.set(&key, &output, TTL_BY_ID).await;

        Ok(output)
    }

    async fn create(&self, group: CreateGroup) -> Result<StaffGroup, DataServiceError> {
        let output = self.inner.create(group).await?;
        self.invalidate_lists().await;

        Ok(output)
    }

    async fn batch_create(
        &self,
        groups: Vec<CreateGroup>,
    ) -> Result<Vec<StaffGroup>, DataServiceError> {
        let output = self.inner.batch_create(groups).await?;
        self.invalidate_lists().await;

        Ok(output)
    }

    async fn update(&self, id: Uuid, group: UpdateGroup) -> Result<StaffGroup, DataServiceError> {
        let output = self.inner.update(id, group).await?;
        self.invalidate_with_membership(id).await;
        Ok(output)
    }

    async fn delete(&self, id: Uuid) -> Result<(), DataServiceError> {
        self.inner.delete(id).await?;
        self.invalidate_all(id).await;

        Ok(())
    }
}
