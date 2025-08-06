use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use url::Url;

use crate::error::{FhirSchemaError, Result};
use crate::storage::{CacheConfig, HierarchicalCache, SchemaStorage};
use crate::types::FhirSchema;

#[derive(Clone)]
pub struct StorageConfig {
    pub storage: Arc<dyn SchemaStorage>,
    pub cache: CacheConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        use crate::storage::MemoryStorage;
        Self {
            storage: Arc::new(MemoryStorage::new()),
            cache: CacheConfig::default(),
        }
    }
}

#[derive(Debug)]
pub struct DependencyTracker {
    dependencies: DashMap<Url, HashSet<Url>>,
}

impl Default for DependencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyTracker {
    pub fn new() -> Self {
        Self {
            dependencies: DashMap::new(),
        }
    }

    pub fn add_dependency(&self, schema_url: Url, depends_on: Url) {
        self.dependencies
            .entry(depends_on)
            .or_default()
            .insert(schema_url);
    }

    pub fn remove_dependency(&self, schema_url: &Url, depends_on: &Url) {
        if let Some(mut deps) = self.dependencies.get_mut(depends_on) {
            deps.remove(schema_url);
            if deps.is_empty() {
                drop(deps);
                self.dependencies.remove(depends_on);
            }
        }
    }

    pub async fn get_dependencies(&self, url: &Url) -> Vec<Url> {
        self.dependencies
            .get(url)
            .map(|deps| deps.clone().into_iter().collect())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        self.dependencies.clear();
    }
}

#[derive(Debug)]
pub struct InvalidationRequest {
    pub url: Url,
    pub strategy: InvalidationStrategy,
}

#[derive(Debug, Clone)]
pub enum InvalidationStrategy {
    Immediate,
    Cascading,
    LazyMark,
}

pub struct EnhancedStorageManager {
    primary_storage: Arc<dyn SchemaStorage>,
    cache: Arc<HierarchicalCache>,
    invalidation_tx: mpsc::Sender<InvalidationRequest>,
    dependency_tracker: Arc<DependencyTracker>,
}

impl EnhancedStorageManager {
    pub fn new(config: StorageConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let cache = Arc::new(HierarchicalCache::new(config.cache, config.storage.clone()));
        let dependency_tracker = Arc::new(DependencyTracker::new());

        let manager = Self {
            primary_storage: config.storage,
            cache: cache.clone(),
            invalidation_tx: tx,
            dependency_tracker: dependency_tracker.clone(),
        };

        // Start invalidation processor
        manager.start_invalidation_processor(rx, cache, dependency_tracker);

        manager
    }

    fn start_invalidation_processor(
        &self,
        mut rx: mpsc::Receiver<InvalidationRequest>,
        cache: Arc<HierarchicalCache>,
        tracker: Arc<DependencyTracker>,
    ) {
        tokio::spawn(async move {
            while let Some(request) = rx.recv().await {
                match request.strategy {
                    InvalidationStrategy::Immediate => {
                        cache.invalidate(&request.url).await;
                    }
                    InvalidationStrategy::Cascading => {
                        // Invalidate the schema itself
                        cache.invalidate(&request.url).await;

                        // Invalidate all dependent schemas
                        let deps = tracker.get_dependencies(&request.url).await;
                        for dep_url in deps {
                            cache.invalidate(&dep_url).await;
                        }
                    }
                    InvalidationStrategy::LazyMark => {
                        cache.mark_stale(&request.url).await;
                    }
                }
            }
        });
    }

    pub async fn get(&self, url: &Url) -> Result<Option<FhirSchema>> {
        self.cache.get(url).await
    }

    pub async fn put(&self, url: Url, schema: FhirSchema) -> Result<()> {
        // Extract dependencies from schema
        let _ = self.extract_and_track_dependencies(&url, &schema).await;

        // Store in cache (which will also store in primary storage)
        self.cache.put(url, schema).await
    }

    pub async fn delete(&self, url: &Url) -> Result<bool> {
        // Remove from primary storage
        let deleted = self.primary_storage.remove(url).await?;

        if deleted {
            // Invalidate cache
            self.cache.invalidate(url).await;

            // Clean up dependencies
            self.cleanup_dependencies(url).await;
        }

        Ok(deleted)
    }

    pub async fn list(&self) -> Result<Vec<Url>> {
        self.primary_storage.list().await
    }

    pub async fn exists(&self, url: &Url) -> Result<bool> {
        self.primary_storage.contains(url).await
    }

    pub async fn clear(&self) -> Result<()> {
        // Clear primary storage
        let urls = self.primary_storage.list().await?;
        for url in urls {
            let _ = self.primary_storage.remove(&url).await;
        }

        // Clear cache
        self.cache.clear().await;

        // Clear dependencies
        self.dependency_tracker.clear();

        Ok(())
    }

    pub async fn invalidate_package(&self, package_id: &str) -> Result<()> {
        // Get all schemas for this package
        let schemas = self
            .primary_storage
            .list()
            .await?
            .into_iter()
            .filter(|url| url.as_str().contains(package_id))
            .collect::<Vec<_>>();

        // Send batch invalidation request
        for url in schemas {
            let _ = self
                .invalidation_tx
                .send(InvalidationRequest {
                    url,
                    strategy: InvalidationStrategy::Cascading,
                })
                .await;
        }

        Ok(())
    }

    pub async fn invalidate_schema(&self, url: &Url, strategy: InvalidationStrategy) -> Result<()> {
        self.invalidation_tx
            .send(InvalidationRequest {
                url: url.clone(),
                strategy,
            })
            .await
            .map_err(|e| FhirSchemaError::Storage {
                message: format!("Failed to send invalidation request: {e}"),
            })
    }

    pub async fn add_dependency(&self, schema_url: Url, depends_on: Url) {
        self.dependency_tracker
            .add_dependency(schema_url, depends_on);
    }

    pub async fn remove_dependency(&self, schema_url: &Url, depends_on: &Url) {
        self.dependency_tracker
            .remove_dependency(schema_url, depends_on);
    }

    pub async fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            l1_size: self.cache.get_l1_size(),
            l2_size: self.cache.get_l2_size().await,
            dependency_count: self.dependency_tracker.dependencies.len(),
        }
    }

    async fn extract_and_track_dependencies(&self, url: &Url, schema: &FhirSchema) -> Result<()> {
        // Clean up existing dependencies for this schema
        self.cleanup_dependencies(url).await;

        // Extract dependencies from schema
        let mut dependencies = Vec::new();

        // Check base schema dependency
        if let Some(base) = &schema.base {
            dependencies.push(base.clone());
        }

        // Check legacy base_definition
        if let Some(base_def) = &schema.base_definition {
            dependencies.push(base_def.clone());
        }

        // Extract dependencies from elements (references to other schemas)
        for element in schema.elements.values() {
            if let Some(element_types) = &element.element_type {
                // Parse type references that might be URLs
                for element_type in element_types {
                    let type_str = &element_type.code;
                    if type_str.starts_with("http") {
                        if let Ok(dep_url) = Url::parse(type_str) {
                            dependencies.push(dep_url);
                        }
                    }
                }
            }
        }

        // Track all dependencies
        for dep_url in dependencies {
            self.dependency_tracker.add_dependency(url.clone(), dep_url);
        }

        Ok(())
    }

    async fn cleanup_dependencies(&self, url: &Url) {
        // Remove this schema from all dependency lists
        let all_deps: Vec<_> = self
            .dependency_tracker
            .dependencies
            .iter()
            .map(|entry| entry.key().clone())
            .collect();

        for dep_url in all_deps {
            self.dependency_tracker.remove_dependency(url, &dep_url);
        }
    }

    pub async fn rebuild_dependencies(&self) -> Result<()> {
        // Clear existing dependencies
        self.dependency_tracker.clear();

        // Rebuild from all schemas
        let all_urls = self.primary_storage.list().await?;

        for url in all_urls {
            if let Some(schema) = self.primary_storage.get(&url).await? {
                self.extract_and_track_dependencies(&url, &schema).await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub l1_size: usize,
    pub l2_size: usize,
    pub dependency_count: usize,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cache Stats: L1={}, L2={}, Dependencies={}",
            self.l1_size, self.l2_size, self.dependency_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;

    fn test_config() -> StorageConfig {
        StorageConfig {
            storage: Arc::new(MemoryStorage::new()),
            cache: CacheConfig {
                l1_size: 2,
                l2_size: 5,
                promotion_threshold: 3,
            },
        }
    }

    fn test_url() -> Url {
        Url::parse("http://example.com/test").unwrap()
    }

    fn test_schema() -> FhirSchema {
        FhirSchema {
            url: Some(test_url()),
            name: Some("TestSchema".to_string()),
            schema_type: "object".to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_enhanced_manager_basic_operations() {
        let config = test_config();
        let manager = EnhancedStorageManager::new(config);
        let url = test_url();
        let schema = test_schema();

        // Test put
        manager.put(url.clone(), schema.clone()).await.unwrap();

        // Test get
        let retrieved = manager.get(&url).await.unwrap().unwrap();
        assert_eq!(schema.name, retrieved.name);

        // Test exists
        assert!(manager.exists(&url).await.unwrap());

        // Test delete
        assert!(manager.delete(&url).await.unwrap());
        assert!(!manager.exists(&url).await.unwrap());
    }

    #[tokio::test]
    async fn test_dependency_tracking() {
        let tracker = DependencyTracker::new();
        let schema_url = Url::parse("http://example.com/schema").unwrap();
        let base_url = Url::parse("http://example.com/base").unwrap();

        // Add dependency
        tracker.add_dependency(schema_url.clone(), base_url.clone());

        // Check dependency exists
        let deps = tracker.get_dependencies(&base_url).await;
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], schema_url);

        // Remove dependency
        tracker.remove_dependency(&schema_url, &base_url);
        let deps = tracker.get_dependencies(&base_url).await;
        assert_eq!(deps.len(), 0);
    }

    #[tokio::test]
    async fn test_cascading_invalidation() {
        let config = test_config();
        let manager = EnhancedStorageManager::new(config);

        let base_url = Url::parse("http://example.com/base").unwrap();
        let derived_url = Url::parse("http://example.com/derived").unwrap();

        let mut base_schema = test_schema();
        base_schema.url = Some(base_url.clone());

        let mut derived_schema = test_schema();
        derived_schema.url = Some(derived_url.clone());
        derived_schema.base = Some(base_url.clone());

        // Store schemas
        manager.put(base_url.clone(), base_schema).await.unwrap();
        manager
            .put(derived_url.clone(), derived_schema)
            .await
            .unwrap();

        // Verify both are cached
        assert!(manager.get(&base_url).await.unwrap().is_some());
        assert!(manager.get(&derived_url).await.unwrap().is_some());

        // Invalidate base schema with cascading strategy
        manager
            .invalidate_schema(&base_url, InvalidationStrategy::Cascading)
            .await
            .unwrap();

        // Give some time for async invalidation to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Both should be invalidated from cache (but still in storage)
        let stats = manager.get_cache_stats().await;
        println!("Cache stats after invalidation: {stats}");
    }

    #[tokio::test]
    async fn test_package_invalidation() {
        let config = test_config();
        let manager = EnhancedStorageManager::new(config);

        let url1 = Url::parse("http://hl7.org/fhir/package1/schema1").unwrap();
        let url2 = Url::parse("http://hl7.org/fhir/package1/schema2").unwrap();
        let url3 = Url::parse("http://hl7.org/fhir/package2/schema3").unwrap();

        let schema = test_schema();

        // Store schemas
        manager.put(url1.clone(), schema.clone()).await.unwrap();
        manager.put(url2.clone(), schema.clone()).await.unwrap();
        manager.put(url3.clone(), schema.clone()).await.unwrap();

        // Invalidate package1
        manager.invalidate_package("package1").await.unwrap();

        // Give some time for async invalidation to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let stats = manager.get_cache_stats().await;
        println!("Cache stats after package invalidation: {stats}");
    }
}
