use crate::error::{FhirSchemaError, Result};
use crate::package::specification::{InstalledPackage, PackageId};
use crate::types::FhirSchema;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use url::Url;

/// Central registry for managing installed packages and their schemas
#[derive(Debug)]
pub struct PackageRegistry {
    /// Installed packages indexed by PackageId
    installed: Arc<DashMap<PackageId, InstalledPackage>>,

    /// Schema index for O(1) schema lookups
    pub schema_index: Arc<SchemaIndex>,

    /// Dependency graph for dependency management
    dependency_graph: Arc<DependencyGraph>,

    /// Package metadata cache
    metadata_cache: Arc<DashMap<PackageId, PackageRegistryMetadata>>,

    /// Registry configuration
    config: RegistryConfig,
}

/// Schema index providing O(1) access to schemas by various keys
#[derive(Debug)]
pub struct SchemaIndex {
    /// Primary index: canonical URL -> FhirSchema
    by_canonical_url: DashMap<String, Arc<FhirSchema>>,

    /// Secondary indexes for efficient lookups
    pub by_resource_type: DashMap<String, Vec<Arc<FhirSchema>>>,
    pub by_package: DashMap<PackageId, Vec<Arc<FhirSchema>>>,
    pub by_profile_type: DashMap<ProfileType, Vec<Arc<FhirSchema>>>,
    pub by_base_type: DashMap<String, Vec<Arc<FhirSchema>>>,

    /// Full-text search index (schema names and descriptions)
    search_index: DashMap<String, Vec<Arc<FhirSchema>>>,

    /// Reverse dependency tracking
    dependencies: DashMap<String, Vec<String>>,

    /// Schema version tracking
    versions: DashMap<String, Vec<SchemaVersion>>,
}

/// Profile type classification for schemas
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProfileType {
    Resource,
    Extension,
    Type,
    Logical,
    Profile,
    Interface,
}

/// Schema version information
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaVersion {
    pub canonical_url: String,
    pub version: String,
    pub package_id: PackageId,
    pub schema: Arc<FhirSchema>,
}

/// Dependency graph for package dependency management
#[derive(Debug)]
pub struct DependencyGraph {
    /// Direct dependencies: package -> its dependencies
    dependencies: DashMap<PackageId, Vec<PackageId>>,

    /// Reverse dependencies: package -> packages that depend on it
    dependents: DashMap<PackageId, Vec<PackageId>>,

    /// Resolved dependency order for installation/uninstallation
    resolution_cache: DashMap<Vec<PackageId>, Vec<PackageId>>,
}

/// Package registry metadata for caching and performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRegistryMetadata {
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
    pub schema_count: usize,
    pub size_bytes: u64,
    pub checksum: Option<String>,
}

/// Registry configuration options
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub max_cached_packages: usize,
    pub index_update_interval: std::time::Duration,
    pub enable_full_text_search: bool,
    pub dependency_resolution_strategy: DependencyStrategy,
    pub schema_validation_level: ValidationLevel,
}

/// Dependency resolution strategy
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyStrategy {
    /// Strict - all dependencies must be satisfied
    Strict,
    /// Best effort - install what's possible, warn about missing
    BestEffort,
    /// Ignore dependencies entirely
    Ignore,
}

/// Schema validation level
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationLevel {
    None,
    Basic,
    Comprehensive,
}

impl PackageRegistry {
    pub fn new(config: RegistryConfig) -> Self {
        Self {
            installed: Arc::new(DashMap::new()),
            schema_index: Arc::new(SchemaIndex::new()),
            dependency_graph: Arc::new(DependencyGraph::new()),
            metadata_cache: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Register an installed package and its schemas
    pub async fn register_package(
        &self,
        package: InstalledPackage,
        schemas: Vec<FhirSchema>,
    ) -> Result<()> {
        let package_id = package.id.clone();

        // Store the package
        self.installed.insert(package_id.clone(), package.clone());

        // Index all schemas from this package - pre-allocate capacity
        let mut schema_refs = Vec::with_capacity(schemas.len());
        for schema in schemas {
            let schema_arc = Arc::new(schema);
            schema_refs.push(schema_arc.clone());
            self.schema_index
                .add_schema(package_id.clone(), schema_arc)
                .await?;
        }

        // Update dependency graph
        self.dependency_graph
            .add_package_dependencies(&package_id, &package.dependencies);

        // Cache metadata
        let metadata = PackageRegistryMetadata {
            last_accessed: Utc::now(),
            access_count: 0,
            schema_count: schema_refs.len(),
            size_bytes: Self::calculate_schemas_size(&schema_refs),
            checksum: package.checksum.clone(),
        };
        self.metadata_cache.insert(package_id, metadata);

        Ok(())
    }

    /// Unregister a package and remove its schemas
    pub async fn unregister_package(
        &self,
        package_id: &PackageId,
    ) -> Result<Option<InstalledPackage>> {
        // Check for dependents
        if let Some(dependents) = self.dependency_graph.get_dependents(package_id) {
            if !dependents.is_empty() {
                return Err(FhirSchemaError::Dependency {
                    message: format!(
                        "Cannot uninstall package {package_id} - it has dependents: {dependents:?}"
                    ),
                });
            }
        }

        // Remove from schema index
        self.schema_index
            .remove_schemas_for_package(package_id)
            .await;

        // Remove from dependency graph
        self.dependency_graph.remove_package(package_id);

        // Remove metadata
        self.metadata_cache.remove(package_id);

        // Remove and return the package
        Ok(self
            .installed
            .remove(package_id)
            .map(|(_, package)| package))
    }

    /// Get schema by canonical URL (O(1) operation)
    pub async fn get_schema(&self, canonical_url: &str) -> Option<Arc<FhirSchema>> {
        let result = self
            .schema_index
            .by_canonical_url
            .get(canonical_url)
            .map(|entry| entry.clone());

        // Update access statistics
        if let Some(schema) = &result {
            self.update_access_stats(&schema.url).await;
        }

        result
    }

    /// Get schemas by resource type
    pub async fn get_schemas_by_type(&self, resource_type: &str) -> Vec<Arc<FhirSchema>> {
        self.schema_index
            .by_resource_type
            .get(resource_type)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Get schemas by profile type
    pub async fn get_schemas_by_profile_type(
        &self,
        profile_type: &ProfileType,
    ) -> Vec<Arc<FhirSchema>> {
        self.schema_index
            .by_profile_type
            .get(profile_type)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Get all schemas for a package
    pub async fn get_package_schemas(&self, package_id: &PackageId) -> Vec<Arc<FhirSchema>> {
        self.schema_index
            .by_package
            .get(package_id)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Search schemas by text query
    pub async fn search_schemas(&self, query: &str) -> Vec<Arc<FhirSchema>> {
        if !self.config.enable_full_text_search {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results = Vec::new(); // Unknown result count, keep as-is

        // Search in index
        for entry in self.schema_index.search_index.iter() {
            if entry.key().to_lowercase().contains(&query_lower) {
                results.extend(entry.value().clone());
            }
        }

        // Deduplicate by canonical URL
        let mut seen = HashSet::new();
        results.retain(|schema| {
            if let Some(url) = &schema.url {
                seen.insert(url.to_string())
            } else {
                true
            }
        });

        results
    }

    /// List all installed packages
    pub fn list_packages(&self) -> Vec<PackageId> {
        self.installed
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get package information
    pub fn get_package(&self, package_id: &PackageId) -> Option<InstalledPackage> {
        self.installed.get(package_id).map(|entry| entry.clone())
    }

    /// Check if a package is installed
    pub fn is_installed(&self, package_id: &PackageId) -> bool {
        self.installed.contains_key(package_id)
    }

    /// Get dependency information
    pub fn get_dependencies(&self, package_id: &PackageId) -> Vec<PackageId> {
        self.dependency_graph
            .get_dependencies(package_id)
            .unwrap_or_default()
    }

    /// Get packages that depend on the given package
    pub fn get_dependents(&self, package_id: &PackageId) -> Vec<PackageId> {
        self.dependency_graph
            .get_dependents(package_id)
            .unwrap_or_default()
    }

    /// Resolve installation order for a set of packages
    pub fn resolve_install_order(&self, packages: &[PackageId]) -> Result<Vec<PackageId>> {
        self.dependency_graph.resolve_install_order(packages)
    }

    /// Get registry statistics
    pub fn get_stats(&self) -> RegistryStats {
        RegistryStats {
            total_packages: self.installed.len(),
            total_schemas: self.schema_index.by_canonical_url.len(),
            memory_usage_mb: self.estimate_memory_usage(),
            cache_hit_rate: self.calculate_cache_hit_rate(),
        }
    }

    /// Get read access to schema index for external use
    pub fn get_schema_index(&self) -> &Arc<SchemaIndex> {
        &self.schema_index
    }

    /// Clear all packages and schemas
    pub async fn clear(&self) -> Result<()> {
        self.installed.clear();
        self.schema_index.clear().await;
        self.dependency_graph.clear();
        self.metadata_cache.clear();
        Ok(())
    }

    /// Update access statistics for a schema
    async fn update_access_stats(&self, _schema_url: &Option<Url>) {
        // Find the package that contains this schema and update its access stats
        if let Some(_url) = _schema_url {
            for mut entry in self.metadata_cache.iter_mut() {
                // This is a simplified approach - in a real implementation,
                // you'd want to maintain a more efficient mapping
                entry.last_accessed = Utc::now();
                entry.access_count += 1;
            }
        }
    }

    /// Calculate the total size of schemas in bytes
    fn calculate_schemas_size(schemas: &[Arc<FhirSchema>]) -> u64 {
        schemas
            .iter()
            .map(|schema| {
                // Estimate size based on serialized JSON representation
                if let Ok(json_str) = serde_json::to_string(schema.as_ref()) {
                    json_str.len() as u64
                } else {
                    // Fallback estimation based on schema properties
                    let mut size = 0u64;

                    // Base schema metadata
                    size += schema.name.as_ref().map_or(0, |n| n.len() as u64);
                    size += schema.title.as_ref().map_or(0, |t| t.len() as u64);
                    size += schema.description.as_ref().map_or(0, |d| d.len() as u64);
                    size += schema.schema_type.len() as u64;

                    // Elements contribute significantly to size
                    size += schema.elements.len() as u64 * 200; // ~200 bytes per element on average

                    // Additional properties based on available fields
                    if let Some(version) = &schema.version {
                        size += version.len() as u64;
                    }
                    if let Some(class) = &schema.class {
                        size += class.len() as u64;
                    }

                    size
                }
            })
            .sum()
    }

    /// Calculate cache hit rate based on access patterns
    fn calculate_cache_hit_rate(&self) -> f64 {
        let mut total_accesses = 0u64;
        let mut total_hits = 0u64;

        // Calculate based on schema access patterns
        for entry in self.metadata_cache.iter() {
            let metadata = entry.value();
            total_accesses += metadata.access_count;

            // Assume schemas accessed more than once had cache hits
            if metadata.access_count > 1 {
                total_hits += metadata.access_count - 1;
            }
        }

        if total_accesses > 0 {
            (total_hits as f64 / total_accesses as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Estimate memory usage in MB
    fn estimate_memory_usage(&self) -> f64 {
        // Rough estimation - in a real implementation you'd want more precise measurement
        let package_count = self.installed.len();
        let schema_count = self.schema_index.by_canonical_url.len();

        // Estimate: ~1KB per package, ~5KB per schema on average
        ((package_count * 1024) + (schema_count * 5 * 1024)) as f64 / (1024.0 * 1024.0)
    }
}

impl Default for SchemaIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaIndex {
    pub fn new() -> Self {
        Self {
            by_canonical_url: DashMap::new(),
            by_resource_type: DashMap::new(),
            by_package: DashMap::new(),
            by_profile_type: DashMap::new(),
            by_base_type: DashMap::new(),
            search_index: DashMap::new(),
            dependencies: DashMap::new(),
            versions: DashMap::new(),
        }
    }

    /// Add a schema to all relevant indexes
    pub async fn add_schema(&self, package_id: PackageId, schema: Arc<FhirSchema>) -> Result<()> {
        // Primary index by canonical URL
        if let Some(url) = &schema.url {
            self.by_canonical_url
                .insert(url.to_string(), schema.clone());
        }

        // Index by resource type
        let resource_type = &schema.schema_type;
        self.by_resource_type
            .entry(resource_type.clone())
            .or_default()
            .push(schema.clone());

        // Index by package
        self.by_package
            .entry(package_id.clone())
            .or_default()
            .push(schema.clone());

        // Index by profile type
        let profile_type = self.determine_profile_type(&schema);
        self.by_profile_type
            .entry(profile_type)
            .or_default()
            .push(schema.clone());

        // Index by base type if available
        if let Some(base) = &schema.base {
            self.by_base_type
                .entry(base.to_string())
                .or_default()
                .push(schema.clone());
        }

        // Full-text search index
        if let Some(name) = &schema.name {
            self.search_index
                .entry(name.clone())
                .or_default()
                .push(schema.clone());
        }

        if let Some(title) = &schema.title {
            self.search_index
                .entry(title.clone())
                .or_default()
                .push(schema.clone());
        }

        // Version tracking
        if let Some(url) = &schema.url {
            let version = SchemaVersion {
                canonical_url: url.to_string(),
                version: schema
                    .version
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                package_id,
                schema: schema.clone(),
            };

            self.versions
                .entry(url.to_string())
                .or_default()
                .push(version);
        }

        Ok(())
    }

    /// Remove all schemas for a package
    pub async fn remove_schemas_for_package(&self, package_id: &PackageId) {
        if let Some((_, schemas)) = self.by_package.remove(package_id) {
            for schema in schemas {
                self.remove_schema_from_indexes(&schema).await;
            }
        }
    }

    /// Clear all indexes
    pub async fn clear(&self) {
        self.by_canonical_url.clear();
        self.by_resource_type.clear();
        self.by_package.clear();
        self.by_profile_type.clear();
        self.by_base_type.clear();
        self.search_index.clear();
        self.dependencies.clear();
        self.versions.clear();
    }

    /// Determine profile type from schema
    fn determine_profile_type(&self, schema: &FhirSchema) -> ProfileType {
        match schema.class.as_deref() {
            Some("resource") => ProfileType::Resource,
            Some("extension") => ProfileType::Extension,
            Some("type") => ProfileType::Type,
            Some("logical") => ProfileType::Logical,
            Some("profile") => ProfileType::Profile,
            _ => {
                // Fallback logic based on schema properties
                if schema.schema_type == "Extension" {
                    ProfileType::Extension
                } else if schema.kind.as_deref() == Some("resource") {
                    ProfileType::Resource
                } else {
                    ProfileType::Type
                }
            }
        }
    }

    /// Remove schema from all secondary indexes
    async fn remove_schema_from_indexes(&self, schema: &Arc<FhirSchema>) {
        // Remove from canonical URL index
        if let Some(url) = &schema.url {
            self.by_canonical_url.remove(&url.to_string());
        }

        // Remove from other indexes - this is simplified,
        // real implementation would need more efficient removal
        // For now, we'd rebuild these indexes periodically or use more sophisticated data structures
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            dependencies: DashMap::new(),
            dependents: DashMap::new(),
            resolution_cache: DashMap::new(),
        }
    }

    pub fn add_package_dependencies(&self, package_id: &PackageId, deps: &[PackageId]) {
        // Add forward dependencies
        self.dependencies.insert(package_id.clone(), deps.to_vec());

        // Add reverse dependencies
        for dep in deps {
            self.dependents
                .entry(dep.clone())
                .or_default()
                .push(package_id.clone());
        }

        // Clear resolution cache as it may be invalidated
        self.resolution_cache.clear();
    }

    pub fn remove_package(&self, package_id: &PackageId) {
        // Remove forward dependencies
        if let Some((_, deps)) = self.dependencies.remove(package_id) {
            // Remove from reverse dependencies
            for dep in deps {
                if let Some(mut dependents) = self.dependents.get_mut(&dep) {
                    dependents.retain(|id| id != package_id);
                }
            }
        }

        // Remove as a dependent
        self.dependents.remove(package_id);

        // Clear resolution cache
        self.resolution_cache.clear();
    }

    pub fn get_dependencies(&self, package_id: &PackageId) -> Option<Vec<PackageId>> {
        self.dependencies.get(package_id).map(|entry| entry.clone())
    }

    pub fn get_dependents(&self, package_id: &PackageId) -> Option<Vec<PackageId>> {
        self.dependents.get(package_id).map(|entry| entry.clone())
    }

    pub fn resolve_install_order(&self, packages: &[PackageId]) -> Result<Vec<PackageId>> {
        // Check cache first
        let cache_key = packages.to_vec();
        if let Some(cached) = self.resolution_cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        // Topological sort for dependency resolution
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        for package_id in packages {
            if !visited.contains(package_id) {
                self.visit_package(package_id, &mut visited, &mut temp_visited, &mut result)?;
            }
        }

        // Cache the result
        self.resolution_cache.insert(cache_key, result.clone());

        Ok(result)
    }

    pub fn clear(&self) {
        self.dependencies.clear();
        self.dependents.clear();
        self.resolution_cache.clear();
    }

    fn visit_package(
        &self,
        package_id: &PackageId,
        visited: &mut HashSet<PackageId>,
        temp_visited: &mut HashSet<PackageId>,
        result: &mut Vec<PackageId>,
    ) -> Result<()> {
        if temp_visited.contains(package_id) {
            return Err(FhirSchemaError::Dependency {
                message: format!("Circular dependency detected involving package {package_id}"),
            });
        }

        if visited.contains(package_id) {
            return Ok(());
        }

        temp_visited.insert(package_id.clone());

        // Visit dependencies first
        if let Some(deps) = self.get_dependencies(package_id) {
            for dep in deps {
                self.visit_package(&dep, visited, temp_visited, result)?;
            }
        }

        temp_visited.remove(package_id);
        visited.insert(package_id.clone());
        result.push(package_id.clone());

        Ok(())
    }
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_packages: usize,
    pub total_schemas: usize,
    pub memory_usage_mb: f64,
    pub cache_hit_rate: f64,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            max_cached_packages: 100,
            index_update_interval: std::time::Duration::from_secs(300), // 5 minutes
            enable_full_text_search: true,
            dependency_resolution_strategy: DependencyStrategy::Strict,
            schema_validation_level: ValidationLevel::Basic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FhirSchema;

    fn create_test_schema(name: &str, url: &str) -> FhirSchema {
        let mut schema = FhirSchema::new(name);
        schema.url = Some(Url::parse(url).unwrap());
        schema.name = Some(name.to_string());
        schema
    }

    #[tokio::test]
    async fn test_package_registry_basic_operations() {
        let config = RegistryConfig::default();
        let registry = PackageRegistry::new(config);

        let package_id = PackageId::new("test.package", "1.0.0");
        let schema = create_test_schema("TestResource", "http://example.com/TestResource");

        // Create installed package
        let installed_package = InstalledPackage {
            id: package_id.clone(),
            spec: crate::package::specification::PackageSpec::registry("test.package", "1.0.0"),
            install_time: Utc::now(),
            file_path: None,
            checksum: None,
            schemas: vec![Url::parse("http://example.com/TestResource").unwrap()],
            dependencies: vec![],
            metadata: Default::default(),
        };

        // Register package
        registry
            .register_package(installed_package, vec![schema.clone()])
            .await
            .unwrap();

        // Test schema retrieval
        let retrieved = registry.get_schema("http://example.com/TestResource").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, schema.name);

        // Test package listing
        let packages = registry.list_packages();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0], package_id);
    }

    #[tokio::test]
    async fn test_dependency_graph() {
        let graph = DependencyGraph::new();
        let pkg_a = PackageId::new("package.a", "1.0.0");
        let pkg_b = PackageId::new("package.b", "1.0.0");
        let pkg_c = PackageId::new("package.c", "1.0.0");

        // A depends on B, B depends on C
        graph.add_package_dependencies(&pkg_a, &[pkg_b.clone()]);
        graph.add_package_dependencies(&pkg_b, &[pkg_c.clone()]);

        // Test dependency resolution
        let install_order = graph.resolve_install_order(&[pkg_a.clone()]).unwrap();

        // Should install in order: C, B, A
        assert_eq!(install_order, vec![pkg_c, pkg_b, pkg_a]);
    }

    #[test]
    fn test_schema_index_profile_type_determination() {
        let index = SchemaIndex::new();

        let mut resource_schema = create_test_schema("Patient", "http://hl7.org/fhir/Patient");
        resource_schema.class = Some("resource".to_string());
        resource_schema.kind = Some("resource".to_string());

        let profile_type = index.determine_profile_type(&resource_schema);
        assert_eq!(profile_type, ProfileType::Resource);
    }
}
