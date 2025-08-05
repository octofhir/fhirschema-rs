use std::sync::Arc;
use url::Url;

use super::{SchemaCache, SchemaStorage};
use crate::{FhirSchema, Result};

pub struct StorageManager {
    primary_storage: Arc<dyn SchemaStorage>,
    cache: Option<Arc<dyn SchemaCache>>,
}

impl StorageManager {
    pub fn new(storage: Arc<dyn SchemaStorage>) -> Self {
        Self {
            primary_storage: storage,
            cache: None,
        }
    }

    pub fn with_cache(mut self, cache: Arc<dyn SchemaCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    pub async fn get(&self, url: &Url) -> Result<Option<FhirSchema>> {
        if let Some(cache) = &self.cache {
            if let Some(schema) = cache.get(url).await {
                return Ok(Some(schema));
            }
        }

        let schema = self.primary_storage.get(url).await?;

        if let (Some(schema), Some(cache)) = (&schema, &self.cache) {
            cache.put(url.clone(), schema.clone()).await;
        }

        Ok(schema)
    }

    pub async fn put(&self, url: Url, schema: FhirSchema) -> Result<()> {
        self.primary_storage
            .put(url.clone(), schema.clone())
            .await?;

        if let Some(cache) = &self.cache {
            cache.put(url, schema).await;
        }

        Ok(())
    }

    pub async fn remove(&self, url: &Url) -> Result<bool> {
        let removed = self.primary_storage.remove(url).await?;

        if let Some(cache) = &self.cache {
            cache.remove(url).await;
        }

        Ok(removed)
    }

    pub async fn list(&self) -> Result<Vec<Url>> {
        self.primary_storage.list().await
    }

    pub async fn contains(&self, url: &Url) -> Result<bool> {
        if let Some(cache) = &self.cache {
            if cache.get(url).await.is_some() {
                return Ok(true);
            }
        }

        self.primary_storage.contains(url).await
    }

    pub async fn clear(&self) -> Result<()> {
        self.primary_storage.clear().await?;

        if let Some(cache) = &self.cache {
            cache.clear().await;
        }

        Ok(())
    }

    pub async fn size(&self) -> Result<usize> {
        self.primary_storage.size().await
    }

    pub fn invalidate_cache(&self, url: &Url) {
        if let Some(cache) = &self.cache {
            tokio::spawn({
                let cache = cache.clone();
                let url = url.clone();
                async move {
                    cache.remove(&url).await;
                }
            });
        }
    }
}
