use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::storage::SchemaStorage;
use crate::types::FhirSchema;

#[derive(Debug)]
pub struct MemoryStorage {
    schemas: Arc<RwLock<HashMap<String, FhirSchema>>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn len(&self) -> usize {
        self.schemas.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.schemas.read().await.is_empty()
    }

    pub async fn clear(&self) {
        self.schemas.write().await.clear();
    }
}

#[async_trait]
impl SchemaStorage for MemoryStorage {
    async fn store_schema(&self, url: &str, schema: FhirSchema) -> Result<()> {
        let mut schemas = self.schemas.write().await;
        schemas.insert(url.to_string(), schema);
        Ok(())
    }

    async fn get_schema(&self, url: &str) -> Result<Option<FhirSchema>> {
        let schemas = self.schemas.read().await;
        Ok(schemas.get(url).cloned())
    }

    async fn list_schemas(&self) -> Result<Vec<String>> {
        let schemas = self.schemas.read().await;
        Ok(schemas.keys().cloned().collect())
    }

    async fn delete_schema(&self, url: &str) -> Result<()> {
        let mut schemas = self.schemas.write().await;
        schemas.remove(url);
        Ok(())
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MemoryStorage {
    fn clone(&self) -> Self {
        Self {
            schemas: Arc::clone(&self.schemas),
        }
    }
}
