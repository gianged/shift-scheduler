use std::sync::Arc;

use async_trait::async_trait;
use shared::types::Staff;
use uuid::Uuid;

use super::client::RedisCache;
use crate::domain::staff::{CreateStaff, StaffRepository, UpdateStaff};
use crate::error::DataServiceError;

/// Cache key for the full staff list.
const KEY_ALL: &str = "data-service:staff:all";
/// TTL in seconds for the full staff list cache entry.
const TTL_ALL: u64 = 300;
/// TTL in seconds for individual staff-by-id cache entries.
const TTL_BY_ID: u64 = 600;

fn key_by_id(id: Uuid) -> String {
    format!("data-service:staff:id:{id}")
}

/// Cache-aside decorator around a [`StaffRepository`].
///
/// Reads check Redis first; writes delegate to the inner repository and
/// invalidate relevant cache keys.
pub struct CachedStaffRepository {
    inner: Arc<dyn StaffRepository>,
    cache: RedisCache,
}

impl CachedStaffRepository {
    pub fn new(inner: Arc<dyn StaffRepository>, cache: RedisCache) -> Self {
        Self { inner, cache }
    }

    async fn invalidate_lists(&self) {
        self.cache.delete(&[KEY_ALL]).await;
    }

    async fn invalidate_all(&self, id: Uuid) {
        self.cache.delete(&[KEY_ALL, &key_by_id(id)]).await;
        self.cache
            .delete_by_pattern("data-service:membership:*")
            .await;
    }
}

#[async_trait]
impl StaffRepository for CachedStaffRepository {
    async fn find_all(&self) -> Result<Vec<Staff>, DataServiceError> {
        if let Some(cached) = self.cache.get::<Vec<Staff>>(KEY_ALL).await {
            return Ok(cached);
        }
        let output = self.inner.find_all().await?;
        self.cache.set(KEY_ALL, &output, TTL_ALL).await;

        Ok(output)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Staff>, DataServiceError> {
        let key = key_by_id(id);
        if let Some(cached) = self.cache.get::<Option<Staff>>(&key).await {
            return Ok(cached);
        }
        let output = self.inner.find_by_id(id).await?;
        self.cache.set(&key, &output, TTL_BY_ID).await;

        Ok(output)
    }

    async fn create(&self, staff: CreateStaff) -> Result<Staff, DataServiceError> {
        let output = self.inner.create(staff).await?;
        self.invalidate_lists().await;

        Ok(output)
    }

    async fn batch_create(&self, staffs: Vec<CreateStaff>) -> Result<Vec<Staff>, DataServiceError> {
        let output = self.inner.batch_create(staffs).await?;
        self.invalidate_lists().await;

        Ok(output)
    }

    async fn update(&self, id: Uuid, staff: UpdateStaff) -> Result<Staff, DataServiceError> {
        let output = self.inner.update(id, staff).await?;
        self.invalidate_all(id).await;

        Ok(output)
    }

    async fn deactivate(&self, id: Uuid) -> Result<(), DataServiceError> {
        self.inner.deactivate(id).await?;
        self.invalidate_all(id).await;

        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), DataServiceError> {
        self.inner.delete(id).await?;
        self.invalidate_all(id).await;

        Ok(())
    }
}
