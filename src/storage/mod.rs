pub mod cache;
pub mod index;
pub mod memory;

#[cfg(feature = "dynamic-caching")]
pub mod disk;

pub use cache::SchemaCache;
pub use memory::MemoryStorage;

#[cfg(feature = "dynamic-caching")]
pub use disk::{CacheStats, DiskStorage, DiskStorageConfig};

use crate::types::FhirSchema;
use async_trait::async_trait;

#[async_trait]
pub trait SchemaStorage: Send + Sync {
    async fn store_schema(&self, url: &str, schema: FhirSchema) -> crate::error::Result<()>;
    async fn get_schema(&self, url: &str) -> crate::error::Result<Option<FhirSchema>>;
    async fn list_schemas(&self) -> crate::error::Result<Vec<String>>;
    async fn delete_schema(&self, url: &str) -> crate::error::Result<()>;
}
