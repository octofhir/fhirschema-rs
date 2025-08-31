use lru::LruCache;
use papaya::HashMap as PapayaMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use url::Url;

use super::{SchemaCache, SchemaStorage, StorageStats};
use crate::{FhirSchema, Result};

#[derive(Debug)]
pub struct MemoryStorage {
    schemas: Arc<PapayaMap<Url, FhirSchema>>,
    stats: Arc<RwLock<StorageStats>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(PapayaMap::new()),
            stats: Arc::new(RwLock::new(StorageStats::default())),
        }
    }

    pub async fn stats(&self) -> StorageStats {
        self.stats.read().await.clone()
    }

    async fn update_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut StorageStats),
    {
        let mut stats = self.stats.write().await;
        updater(&mut stats);
        stats.last_accessed = Some(SystemTime::now());
    }
}

#[async_trait::async_trait]
impl SchemaStorage for MemoryStorage {
    async fn get(&self, url: &Url) -> Result<Option<FhirSchema>> {
        self.update_stats(|s| s.storage_operations += 1).await;

        let guard = self.schemas.pin_owned();
        let result = guard.get(url).cloned();
        Ok(result)
    }

    async fn put(&self, url: Url, schema: FhirSchema) -> Result<()> {
        let exists = {
            let guard = self.schemas.pin_owned();
            guard.get(&url).is_some()
        };

        self.update_stats(|s| {
            s.storage_operations += 1;
            if !exists {
                s.schemas_count += 1;
            }
        })
        .await;

        let guard = self.schemas.pin_owned();
        guard.insert(url, schema);
        Ok(())
    }

    async fn remove(&self, url: &Url) -> Result<bool> {
        self.update_stats(|s| s.storage_operations += 1).await;

        let removed = {
            let guard = self.schemas.pin_owned();
            guard.remove(url).is_some()
        };

        if removed {
            self.update_stats(|s| s.schemas_count = s.schemas_count.saturating_sub(1))
                .await;
        }
        Ok(removed)
    }

    async fn list(&self) -> Result<Vec<Url>> {
        self.update_stats(|s| s.storage_operations += 1).await;

        let guard = self.schemas.pin_owned();
        Ok(guard.iter().map(|(key, _)| key.clone()).collect())
    }

    async fn contains(&self, url: &Url) -> Result<bool> {
        let guard = self.schemas.pin_owned();
        Ok(guard.get(url).is_some())
    }

    async fn clear(&self) -> Result<()> {
        self.update_stats(|s| {
            s.storage_operations += 1;
            s.schemas_count = 0;
        })
        .await;

        let guard = self.schemas.pin_owned();
        guard.clear();
        Ok(())
    }

    async fn size(&self) -> Result<usize> {
        let guard = self.schemas.pin_owned();
        Ok(guard.len())
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct LruSchemaCache {
    cache: Arc<RwLock<LruCache<Url, FhirSchema>>>,
    stats: Arc<RwLock<StorageStats>>,
}

impl LruSchemaCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity.try_into().unwrap()))),
            stats: Arc::new(RwLock::new(StorageStats::default())),
        }
    }

    pub async fn stats(&self) -> StorageStats {
        self.stats.read().await.clone()
    }

    async fn update_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut StorageStats),
    {
        let mut stats = self.stats.write().await;
        updater(&mut stats);
        stats.last_accessed = Some(SystemTime::now());
    }
}

#[async_trait::async_trait]
impl SchemaCache for LruSchemaCache {
    async fn get(&self, url: &Url) -> Option<FhirSchema> {
        let result = self.cache.write().await.get(url).cloned();

        self.update_stats(|s| {
            if result.is_some() {
                s.cache_hits += 1;
            } else {
                s.cache_misses += 1;
            }
        })
        .await;

        result
    }

    async fn put(&self, url: Url, schema: FhirSchema) {
        self.update_stats(|s| s.storage_operations += 1).await;
        self.cache.write().await.put(url, schema);
    }

    async fn remove(&self, url: &Url) -> bool {
        self.update_stats(|s| s.storage_operations += 1).await;
        self.cache.write().await.pop(url).is_some()
    }

    async fn clear(&self) {
        self.update_stats(|s| {
            s.storage_operations += 1;
            s.schemas_count = 0;
        })
        .await;
        self.cache.write().await.clear();
    }

    async fn size(&self) -> usize {
        self.cache.read().await.len()
    }
}
