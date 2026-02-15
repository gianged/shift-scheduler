use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct RedisCache {
    conn: ConnectionManager,
}

impl RedisCache {
    pub async fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;

        Ok(Self { conn })
    }

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut conn = self.conn.clone();
        let output: Result<Option<String>, _> = conn.get(key).await;

        match output {
            Ok(Some(json)) => match serde_json::from_str(&json) {
                Ok(value) => {
                    tracing::info!("Cache hit: {key}");
                    Some(value)
                }
                Err(e) => {
                    tracing::warn!("Cache deserialize error for {key}: {e}");
                    None
                }
            },
            Ok(None) => {
                tracing::info!("Cache miss: {key}");
                None
            }
            Err(e) => {
                tracing::warn!("Cache get error for {key}: {e}");
                None
            }
        }
    }

    pub async fn set<T: Serialize>(&self, key: &str, value: &T, ttl_seconds: u64) {
        let mut conn = self.conn.clone();

        match serde_json::to_string(value) {
            Ok(json) => {
                let output: Result<(), _> = conn.set_ex(key, &json, ttl_seconds).await;
                if let Err(e) = output {
                    tracing::warn!("Cache set error for {key}: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("Cache serialize error for {key}: {e}");
            }
        }
    }

    pub async fn delete(&self, keys: &[&str]) {
        if keys.is_empty() {
            return;
        }
        let mut conn = self.conn.clone();
        let output: Result<(), _> = conn.del(keys).await;
        if let Err(e) = output {
            tracing::warn!("Cache delete error for {keys:?}: {e}");
        }
    }

    pub async fn delete_by_pattern(&self, pattern: &str) {
        let mut conn = self.conn.clone();
        let mut cursor: u64 = 0;
        let mut keys_to_delete: Vec<String> = Vec::new();

        loop {
            let output: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await;

            match output {
                Ok((next_cursor, keys)) => {
                    keys_to_delete.extend(keys);
                    cursor = next_cursor;
                    if cursor == 0 {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Cache scan error for pattern {pattern}: {e}");

                    return;
                }
            }
        }

        if !keys_to_delete.is_empty() {
            let output: Result<(), _> = conn.del(&keys_to_delete).await;
            if let Err(e) = output {
                tracing::warn!("cache pattern delete error for {pattern}: {e}");
            }
        }
    }
}
