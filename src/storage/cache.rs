use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::CacheConfig;
use crate::error::Result;
use crate::types::FhirSchema;

pub struct SchemaCache {
    l1_cache: Arc<RwLock<LruCache<String, FhirSchema>>>,
    config: CacheConfig,
}

impl SchemaCache {
    pub async fn new(config: &CacheConfig) -> Result<Self> {
        let l1_size = NonZeroUsize::new(config.l1_size).ok_or_else(|| {
            crate::error::FhirSchemaError::configuration_error("L1 cache size cannot be zero")
        })?;

        Ok(Self {
            l1_cache: Arc::new(RwLock::new(LruCache::new(l1_size))),
            config: config.clone(),
        })
    }

    pub async fn get(&self, key: &str) -> Option<FhirSchema> {
        let mut cache = self.l1_cache.write().await;
        cache.get(key).cloned()
    }

    pub async fn insert(&self, key: &str, schema: &FhirSchema) -> Result<()> {
        let mut cache = self.l1_cache.write().await;
        cache.put(key.to_string(), schema.clone());
        Ok(())
    }

    pub async fn remove(&self, key: &str) -> Option<FhirSchema> {
        let mut cache = self.l1_cache.write().await;
        cache.pop(key)
    }

    pub async fn clear(&self) {
        let mut cache = self.l1_cache.write().await;
        cache.clear();
    }

    pub async fn len(&self) -> usize {
        let cache = self.l1_cache.read().await;
        cache.len()
    }

    pub async fn is_empty(&self) -> bool {
        let cache = self.l1_cache.read().await;
        cache.is_empty()
    }

    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
}

impl std::fmt::Debug for SchemaCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemaCache")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
