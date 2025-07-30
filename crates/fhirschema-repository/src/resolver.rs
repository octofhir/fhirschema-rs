//! Schema reference resolution and caching for FHIRSchema repository

use async_trait::async_trait;
use fhirschema_core::{Schema, Element};
use lru::LruCache;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

use crate::{RepositoryError, RepositoryResult, SchemaRepository, SchemaVersion};

/// Schema resolver for handling reference resolution and caching
pub struct SchemaResolver {
    /// Primary repository for schema resolution
    repository: Arc<dyn SchemaRepository>,
    /// LRU cache for frequently accessed schemas
    cache: Arc<RwLock<LruCache<String, CachedSchema>>>,
    /// Dependency graph for tracking schema relationships
    dependency_graph: Arc<RwLock<DependencyGraph>>,
    /// Configuration for resolver behavior
    config: ResolverConfig,
}

/// Cached schema entry
#[derive(Debug, Clone)]
struct CachedSchema {
    /// The cached schema
    schema: Schema,
    /// Cache timestamp
    cached_at: std::time::Instant,
    /// Schema version
    version: SchemaVersion,
    /// Dependencies of this schema
    dependencies: Vec<String>,
}

/// Dependency graph for tracking schema relationships
#[derive(Debug, Default)]
struct DependencyGraph {
    /// Map from schema URL to its dependencies
    dependencies: HashMap<String, HashSet<String>>,
    /// Map from schema URL to schemas that depend on it
    dependents: HashMap<String, HashSet<String>>,
}

/// Configuration for schema resolver
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// Maximum number of schemas to cache
    pub cache_size: usize,
    /// Cache TTL (time to live) in seconds
    pub cache_ttl: u64,
    /// Maximum depth for dependency resolution
    pub max_dependency_depth: usize,
    /// Enable circular dependency detection
    pub detect_circular_deps: bool,
    /// Enable remote schema fetching
    pub enable_remote_fetch: bool,
    /// Timeout for remote fetching in seconds
    pub remote_timeout: u64,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            cache_size: 1000,
            cache_ttl: 3600, // 1 hour
            max_dependency_depth: 10,
            detect_circular_deps: true,
            enable_remote_fetch: false,
            remote_timeout: 30,
        }
    }
}

impl SchemaResolver {
    /// Create a new schema resolver
    pub fn new(repository: Arc<dyn SchemaRepository>) -> Self {
        Self::with_config(repository, ResolverConfig::default())
    }

    /// Create a new schema resolver with custom configuration
    pub fn with_config(repository: Arc<dyn SchemaRepository>, config: ResolverConfig) -> Self {
        let cache_size = NonZeroUsize::new(config.cache_size).unwrap_or(NonZeroUsize::new(1000).unwrap());

        Self {
            repository,
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            dependency_graph: Arc::new(RwLock::new(DependencyGraph::default())),
            config,
        }
    }

    /// Resolve a schema by its canonical URL
    pub async fn resolve_schema(&self, url: &str) -> RepositoryResult<Schema> {
        self.resolve_schema_with_version(url, None).await
    }

    /// Resolve a specific version of a schema
    pub async fn resolve_schema_with_version(
        &self,
        url: &str,
        version: Option<&SchemaVersion>,
    ) -> RepositoryResult<Schema> {
        let cache_key = match version {
            Some(v) => format!("{}@{}", url, v),
            None => url.to_string(),
        };

        // Check cache first
        if let Some(cached) = self.get_from_cache(&cache_key).await {
            if !self.is_cache_expired(&cached) {
                return Ok(cached.schema);
            }
        }

        // Resolve from repository
        let schema = match version {
            Some(v) => self.repository.get_schema_version(url, v).await?,
            None => self.repository.get_latest_schema(url).await?,
        };

        // Extract dependencies from schema
        let dependencies = self.extract_dependencies(&schema);

        // Update dependency graph
        self.update_dependency_graph(url, &dependencies).await;

        // Cache the resolved schema
        let cached_schema = CachedSchema {
            schema: schema.clone(),
            cached_at: std::time::Instant::now(),
            version: version.cloned().unwrap_or_default(),
            dependencies,
        };

        self.cache_schema(cache_key, cached_schema).await;

        Ok(schema)
    }

    /// Resolve all dependencies of a schema recursively
    pub async fn resolve_dependencies(&self, url: &str) -> RepositoryResult<Vec<Schema>> {
        let mut resolved = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();

        self.resolve_dependencies_recursive(url, &mut resolved, &mut visited, &mut stack, 0).await?;

        Ok(resolved)
    }

    /// Check for circular dependencies in the schema graph
    pub async fn check_circular_dependencies(&self, url: &str) -> RepositoryResult<Option<Vec<String>>> {
        if !self.config.detect_circular_deps {
            return Ok(None);
        }

        let mut visited = HashSet::new();
        let mut stack = Vec::new();

        if let Some(cycle) = self.detect_cycle_recursive(url, &mut visited, &mut stack).await? {
            Ok(Some(cycle))
        } else {
            Ok(None)
        }
    }

    /// Get dependency graph for a schema
    pub async fn get_dependency_graph(&self, url: &str) -> RepositoryResult<DependencyInfo> {
        let graph = self.dependency_graph.read().await;

        let dependencies = graph.dependencies.get(url).cloned().unwrap_or_default();
        let dependents = graph.dependents.get(url).cloned().unwrap_or_default();

        Ok(DependencyInfo {
            url: url.to_string(),
            dependencies: dependencies.into_iter().collect(),
            dependents: dependents.into_iter().collect(),
        })
    }

    /// Clear the resolver cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get cache statistics
    pub async fn cache_statistics(&self) -> CacheStatistics {
        let cache = self.cache.read().await;
        let graph = self.dependency_graph.read().await;

        CacheStatistics {
            cache_size: cache.len(),
            cache_capacity: cache.cap().get(),
            dependency_count: graph.dependencies.len(),
            dependent_count: graph.dependents.len(),
        }
    }

    /// Invalidate cache entries for a specific schema
    pub async fn invalidate_cache(&self, url: &str) {
        let mut cache = self.cache.write().await;

        // Remove all cache entries that match the URL (with or without version)
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter_map(|(key, _)| {
                if key == url || key.starts_with(&format!("{}@", url)) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            cache.pop(&key);
        }
    }

    // Private helper methods

    async fn get_from_cache(&self, key: &str) -> Option<CachedSchema> {
        let mut cache = self.cache.write().await;
        cache.get(key).cloned()
    }

    async fn cache_schema(&self, key: String, schema: CachedSchema) {
        let mut cache = self.cache.write().await;
        cache.put(key, schema);
    }

    fn is_cache_expired(&self, cached: &CachedSchema) -> bool {
        let ttl = std::time::Duration::from_secs(self.config.cache_ttl);
        cached.cached_at.elapsed() > ttl
    }

    fn extract_dependencies(&self, schema: &Schema) -> Vec<String> {
        let mut dependencies = Vec::new();

        // Extract base schema reference
        if let Some(base) = &schema.base {
            dependencies.push(base.clone());
        }

        // Extract element type references
        if let Some(elements) = &schema.elements {
            for element in elements.values() {
                self.extract_element_dependencies(element, &mut dependencies);
            }
        }

        // Remove duplicates and self-references
        dependencies.sort();
        dependencies.dedup();
        dependencies.retain(|dep| dep != &schema.url);

        dependencies
    }

    fn extract_element_dependencies(&self, element: &Element, dependencies: &mut Vec<String>) {
        // Extract type reference
        if let Some(element_type) = &element.element_type {
            dependencies.push(element_type.clone());
        }

        // Extract reference targets
        if let Some(refers) = &element.refers {
            dependencies.extend(refers.iter().cloned());
        }

        // Extract from choice types
        if let Some(choices) = &element.choices {
            for choice_element in choices.values() {
                self.extract_element_dependencies(choice_element, dependencies);
            }
        }

        // Extract from nested elements
        if let Some(nested_elements) = &element.elements {
            for nested_element in nested_elements.values() {
                self.extract_element_dependencies(nested_element, dependencies);
            }
        }
    }

    async fn update_dependency_graph(&self, url: &str, dependencies: &[String]) {
        let mut graph = self.dependency_graph.write().await;

        // Update dependencies
        graph.dependencies.insert(url.to_string(), dependencies.iter().cloned().collect());

        // Update dependents
        for dep in dependencies {
            graph.dependents
                .entry(dep.clone())
                .or_insert_with(HashSet::new)
                .insert(url.to_string());
        }
    }

    async fn resolve_dependencies_recursive(
        &self,
        url: &str,
        resolved: &mut Vec<Schema>,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> RepositoryResult<()> {
        if depth > self.config.max_dependency_depth {
            return Err(RepositoryError::generic(
                format!("Maximum dependency depth {} exceeded", self.config.max_dependency_depth)
            ));
        }

        if visited.contains(url) {
            return Ok(());
        }

        if stack.contains(&url.to_string()) {
            return Err(RepositoryError::circular_dependency(
                stack.join(" -> ") + " -> " + url
            ));
        }

        stack.push(url.to_string());
        visited.insert(url.to_string());

        let schema = self.resolve_schema(url).await?;
        let dependencies = self.extract_dependencies(&schema);

        for dep in &dependencies {
            Box::pin(self.resolve_dependencies_recursive(dep, resolved, visited, stack, depth + 1)).await?;
        }

        resolved.push(schema);
        stack.pop();

        Ok(())
    }

    async fn detect_cycle_recursive(
        &self,
        url: &str,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
    ) -> RepositoryResult<Option<Vec<String>>> {
        if stack.contains(&url.to_string()) {
            // Found a cycle - return the cycle path
            let cycle_start = stack.iter().position(|s| s == url).unwrap();
            let mut cycle = stack[cycle_start..].to_vec();
            cycle.push(url.to_string());
            return Ok(Some(cycle));
        }

        if visited.contains(url) {
            return Ok(None);
        }

        visited.insert(url.to_string());
        stack.push(url.to_string());

        // Get dependencies from cache or resolve
        let dependencies = if let Some(cached) = self.get_from_cache(url).await {
            cached.dependencies
        } else {
            let schema = self.resolve_schema(url).await?;
            self.extract_dependencies(&schema)
        };

        for dep in &dependencies {
            if let Some(cycle) = Box::pin(self.detect_cycle_recursive(dep, visited, stack)).await? {
                return Ok(Some(cycle));
            }
        }

        stack.pop();
        Ok(None)
    }
}

/// Dependency information for a schema
#[derive(Debug, Clone)]
pub struct DependencyInfo {
    /// Schema URL
    pub url: String,
    /// List of schemas this schema depends on
    pub dependencies: Vec<String>,
    /// List of schemas that depend on this schema
    pub dependents: Vec<String>,
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStatistics {
    /// Current cache size
    pub cache_size: usize,
    /// Maximum cache capacity
    pub cache_capacity: usize,
    /// Number of tracked dependencies
    pub dependency_count: usize,
    /// Number of tracked dependents
    pub dependent_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryRepository;
    use fhirschema_core::{Element, Schema};

    #[tokio::test]
    async fn test_schema_resolution() {
        let repo = Arc::new(MemoryRepository::new());
        let resolver = SchemaResolver::new(repo.clone());

        // Create a test schema
        let schema = Schema::new(
            "http://example.com/test".to_string(),
            "Resource".to_string(),
            "Test".to_string(),
            "specialization".to_string(),
        );

        // Store schema in repository
        repo.store_schema(&schema, None).await.unwrap();

        // Resolve schema
        let resolved = resolver.resolve_schema("http://example.com/test").await.unwrap();
        assert_eq!(resolved.url, "http://example.com/test");
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let repo = Arc::new(MemoryRepository::new());
        let resolver = SchemaResolver::new(repo.clone());

        let schema = Schema::new(
            "http://example.com/cached".to_string(),
            "Resource".to_string(),
            "Cached".to_string(),
            "specialization".to_string(),
        );

        repo.store_schema(&schema, None).await.unwrap();

        // First resolution - should cache
        let _resolved1 = resolver.resolve_schema("http://example.com/cached").await.unwrap();

        // Second resolution - should use cache
        let _resolved2 = resolver.resolve_schema("http://example.com/cached").await.unwrap();

        let stats = resolver.cache_statistics().await;
        assert_eq!(stats.cache_size, 1);
    }
}
