use async_trait::async_trait;
use url::Url;

use crate::{FhirSchema, Result};

#[async_trait]
pub trait SchemaStorage: Send + Sync {
    async fn get(&self, url: &Url) -> Result<Option<FhirSchema>>;
    async fn put(&self, url: Url, schema: FhirSchema) -> Result<()>;
    async fn remove(&self, url: &Url) -> Result<bool>;
    async fn list(&self) -> Result<Vec<Url>>;
    async fn contains(&self, url: &Url) -> Result<bool>;
    async fn clear(&self) -> Result<()>;
    async fn size(&self) -> Result<usize>;
}

#[async_trait]
pub trait SchemaCache: Send + Sync {
    async fn get(&self, url: &Url) -> Option<FhirSchema>;
    async fn put(&self, url: Url, schema: FhirSchema);
    async fn remove(&self, url: &Url) -> bool;
    async fn clear(&self);
    async fn size(&self) -> usize;
}

pub trait SchemaResolver: Send + Sync {
    fn resolve(&self, base_url: &Url, reference: &str) -> Result<Url>;
    fn is_absolute(&self, reference: &str) -> bool;
    fn normalize_url(&self, url: &Url) -> Url;
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct StorageStats {
    pub schemas_count: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub storage_operations: u64,
    pub last_accessed: Option<std::time::SystemTime>,
}

