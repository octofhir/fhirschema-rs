use crate::error::{FhirSchemaError, Result};
use crate::package::specification::{InstalledPackage, PackageId};
use crate::types::{FhirSchema, ResourceTypeRegistry};
use chrono::{DateTime, Utc};
use papaya::HashMap as PapayaMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use url::Url;

/// Central registry for managing installed packages and their schemas
#[derive(Debug)]
pub struct PackageRegistry {
    /// Installed packages indexed by PackageId
    installed: Arc<PapayaMap<PackageId, InstalledPackage>>,

    /// Schema index for O(1) schema lookups
    pub schema_index: Arc<SchemaIndex>,

    /// Dedicated type registry for O(1) type access (atomic replacement)
    pub type_registry: Arc<papaya::HashMap<(), ResourceTypeRegistry>>,

    /// Dependency graph for dependency management
    dependency_graph: Arc<DependencyGraph>,

    /// Package metadata cache
    metadata_cache: Arc<PapayaMap<PackageId, PackageRegistryMetadata>>,

    /// Registry configuration
    config: RegistryConfig,
}

/// Schema index providing O(1) access to schemas by various keys
#[derive(Debug)]
pub struct SchemaIndex {
    /// Primary index: canonical URL -> FhirSchema
    by_canonical_url: PapayaMap<String, Arc<FhirSchema>>,

    /// Secondary indexes for efficient lookups
    pub by_resource_type: PapayaMap<String, Vec<Arc<FhirSchema>>>,
    pub by_package: PapayaMap<PackageId, Vec<Arc<FhirSchema>>>,
    pub by_profile_type: PapayaMap<ProfileType, Vec<Arc<FhirSchema>>>,
    pub by_base_type: PapayaMap<String, Vec<Arc<FhirSchema>>>,

    /// Full-text search index (schema names and descriptions)
    search_index: PapayaMap<String, Vec<Arc<FhirSchema>>>,

    /// Reverse dependency tracking
    dependencies: PapayaMap<String, Vec<String>>,

    /// Schema version tracking
    versions: PapayaMap<String, Vec<SchemaVersion>>,
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
    dependencies: PapayaMap<PackageId, Vec<PackageId>>,

    /// Reverse dependencies: package -> packages that depend on it
    dependents: PapayaMap<PackageId, Vec<PackageId>>,

    /// Resolved dependency order for installation/uninstallation
    resolution_cache: PapayaMap<Vec<PackageId>, Vec<PackageId>>,
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
            installed: Arc::new(PapayaMap::new()),
            schema_index: Arc::new(SchemaIndex::new()),
            type_registry: {
                let registry_map = papaya::HashMap::new();
                {
                    let guard = registry_map.pin();
                    guard.insert((), ResourceTypeRegistry::new());
                }
                Arc::new(registry_map)
            },
            dependency_graph: Arc::new(DependencyGraph::new()),
            metadata_cache: Arc::new(PapayaMap::new()),
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
        let guard = self.installed.pin();
        guard.insert(package_id.clone(), package.clone());

        // Index all schemas from this package - pre-allocate capacity
        let mut schema_refs = Vec::with_capacity(schemas.len());
        for schema in schemas {
            let schema_arc = Arc::new(schema);
            schema_refs.push(schema_arc.clone());
            self.schema_index
                .add_schema(package_id.clone(), schema_arc)
                .await?;
        }

        // Rebuild type registry when schemas change
        self.rebuild_type_registry().await?;

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
        let cache_guard = self.metadata_cache.pin();
        cache_guard.insert(package_id, metadata);

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
        let cache_guard = self.metadata_cache.pin();
        cache_guard.remove(package_id);

        // Remove and return the package
        let installed_guard = self.installed.pin();
        Ok(installed_guard.remove(package_id).cloned())
    }

    /// Rebuild type registry when schemas change
    pub async fn rebuild_type_registry(&self) -> Result<()> {
        let schemas = self.get_all_schemas().await;
        let new_registry = ResourceTypeRegistry::build_from_schemas(&schemas).await?;

        // Replace the registry atomically using papaya HashMap
        let guard = self.type_registry.pin();
        guard.insert((), new_registry);

        Ok(())
    }

    /// Get all schemas across all packages
    pub async fn get_all_schemas(&self) -> Vec<Arc<FhirSchema>> {
        let guard = self.schema_index.by_canonical_url.pin();
        guard.iter().map(|(_, schema)| schema.clone()).collect()
    }

    /// Get schema by canonical URL (O(1) operation)
    pub async fn get_schema(&self, canonical_url: &str) -> Option<Arc<FhirSchema>> {
        let result = {
            let guard = self.schema_index.by_canonical_url.pin();
            guard.get(canonical_url).cloned()
        };

        // Update access statistics
        if let Some(schema) = &result {
            self.update_access_stats(&schema.url).await;
        }

        result
    }

    /// Get schemas by resource type
    pub async fn get_schemas_by_type(&self, resource_type: &str) -> Vec<Arc<FhirSchema>> {
        let guard = self.schema_index.by_resource_type.pin();
        guard.get(resource_type).cloned().unwrap_or_default()
    }

    /// Get schemas by profile type
    pub async fn get_schemas_by_profile_type(
        &self,
        profile_type: &ProfileType,
    ) -> Vec<Arc<FhirSchema>> {
        let guard = self.schema_index.by_profile_type.pin();
        guard.get(profile_type).cloned().unwrap_or_default()
    }

    /// Get all schemas for a package
    pub async fn get_package_schemas(&self, package_id: &PackageId) -> Vec<Arc<FhirSchema>> {
        let guard = self.schema_index.by_package.pin();
        guard.get(package_id).cloned().unwrap_or_default()
    }

    /// Search schemas by text query
    pub async fn search_schemas(&self, query: &str) -> Vec<Arc<FhirSchema>> {
        if !self.config.enable_full_text_search {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results = Vec::new(); // Unknown result count, keep as-is

        // Search in index
        let search_guard = self.schema_index.search_index.pin();
        for (key, value) in search_guard.iter() {
            if key.to_lowercase().contains(&query_lower) {
                results.extend(value.clone());
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
        let guard = self.installed.pin();
        guard.iter().map(|(key, _)| key.clone()).collect()
    }

    /// Get package information
    pub fn get_package(&self, package_id: &PackageId) -> Option<InstalledPackage> {
        let guard = self.installed.pin();
        guard.get(package_id).cloned()
    }

    /// Check if a package is installed
    pub fn is_installed(&self, package_id: &PackageId) -> bool {
        let guard = self.installed.pin();
        guard.get(package_id).is_some()
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
        let installed_guard = self.installed.pin();
        let schemas_guard = self.schema_index.by_canonical_url.pin();
        RegistryStats {
            total_packages: installed_guard.len(),
            total_schemas: schemas_guard.len(),
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
        let installed_guard = self.installed.pin();
        installed_guard.clear();
        self.schema_index.clear().await;
        self.dependency_graph.clear();
        let cache_guard = self.metadata_cache.pin();
        cache_guard.clear();
        Ok(())
    }

    /// Update access statistics for a schema
    async fn update_access_stats(&self, _schema_url: &Option<Url>) {
        // Find the package that contains this schema and update its access stats
        if let Some(_url) = _schema_url {
            // For papaya, we'll need to implement more sophisticated access tracking
            // since there's no iter_mut - this would require atomic operations
            // or a different approach to access statistics
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
        let cache_guard = self.metadata_cache.pin();
        for (_, metadata) in cache_guard.iter() {
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
        let installed_guard = self.installed.pin();
        let schemas_guard = self.schema_index.by_canonical_url.pin();
        let package_count = installed_guard.len();
        let schema_count = schemas_guard.len();

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
            by_canonical_url: PapayaMap::new(),
            by_resource_type: PapayaMap::new(),
            by_package: PapayaMap::new(),
            by_profile_type: PapayaMap::new(),
            by_base_type: PapayaMap::new(),
            search_index: PapayaMap::new(),
            dependencies: PapayaMap::new(),
            versions: PapayaMap::new(),
        }
    }

    /// Add a schema to all relevant indexes
    pub async fn add_schema(&self, package_id: PackageId, schema: Arc<FhirSchema>) -> Result<()> {
        // Primary index by canonical URL
        if let Some(url) = &schema.url {
            let guard = self.by_canonical_url.pin();
            guard.insert(url.to_string(), schema.clone());
        }

        // Index by resource type
        let resource_type = &schema.schema_type;
        let guard = self.by_resource_type.pin();
        let mut schemas = guard.get(resource_type).cloned().unwrap_or_default();
        schemas.push(schema.clone());
        guard.insert(resource_type.clone(), schemas);

        // Index by package
        let guard = self.by_package.pin();
        let mut schemas = guard.get(&package_id).cloned().unwrap_or_default();
        schemas.push(schema.clone());
        guard.insert(package_id.clone(), schemas);

        // Index by profile type
        let profile_type = self.determine_profile_type(&schema);
        let guard = self.by_profile_type.pin();
        let mut schemas = guard.get(&profile_type).cloned().unwrap_or_default();
        schemas.push(schema.clone());
        guard.insert(profile_type, schemas);

        // Index by base type if available
        if let Some(base) = &schema.base {
            let guard = self.by_base_type.pin();
            let base_str = base.to_string();
            let mut schemas = guard.get(&base_str).cloned().unwrap_or_default();
            schemas.push(schema.clone());
            guard.insert(base_str, schemas);
        }

        // Full-text search index
        if let Some(name) = &schema.name {
            let guard = self.search_index.pin();
            let mut schemas = guard.get(name).cloned().unwrap_or_default();
            schemas.push(schema.clone());
            guard.insert(name.clone(), schemas);
        }

        if let Some(title) = &schema.title {
            let guard = self.search_index.pin();
            let mut schemas = guard.get(title).cloned().unwrap_or_default();
            schemas.push(schema.clone());
            guard.insert(title.clone(), schemas);
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

            let guard = self.versions.pin();
            let mut versions = guard.get(&url.to_string()).cloned().unwrap_or_default();
            versions.push(version);
            guard.insert(url.to_string(), versions);
        }

        Ok(())
    }

    /// Remove all schemas for a package
    pub async fn remove_schemas_for_package(&self, package_id: &PackageId) {
        let guard = self.by_package.pin();
        if let Some(schemas) = guard.remove(package_id) {
            for schema in schemas {
                self.remove_schema_from_indexes(schema).await;
            }
        }
    }

    /// Clear all indexes
    pub async fn clear(&self) {
        let canonical_guard = self.by_canonical_url.pin();
        canonical_guard.clear();
        let resource_guard = self.by_resource_type.pin();
        resource_guard.clear();
        let package_guard = self.by_package.pin();
        package_guard.clear();
        let profile_guard = self.by_profile_type.pin();
        profile_guard.clear();
        let base_guard = self.by_base_type.pin();
        base_guard.clear();
        let search_guard = self.search_index.pin();
        search_guard.clear();
        let deps_guard = self.dependencies.pin();
        deps_guard.clear();
        let versions_guard = self.versions.pin();
        versions_guard.clear();
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

    /// Get first schema by resource type (for path resolution)
    pub async fn get_schema_by_type(&self, resource_type: &str) -> Option<Arc<FhirSchema>> {
        let guard = self.by_resource_type.pin();
        guard.get(resource_type)?.first().cloned()
    }

    /// Remove schema from all secondary indexes
    async fn remove_schema_from_indexes(&self, schema: &Arc<FhirSchema>) {
        // Remove from canonical URL index
        if let Some(url) = &schema.url {
            let guard = self.by_canonical_url.pin();
            guard.remove(&url.to_string());
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
            dependencies: PapayaMap::new(),
            dependents: PapayaMap::new(),
            resolution_cache: PapayaMap::new(),
        }
    }

    pub fn add_package_dependencies(&self, package_id: &PackageId, deps: &[PackageId]) {
        // Add forward dependencies
        let deps_guard = self.dependencies.pin();
        deps_guard.insert(package_id.clone(), deps.to_vec());

        // Add reverse dependencies
        let dependents_guard = self.dependents.pin();
        for dep in deps {
            let mut dependents_list = dependents_guard.get(dep).cloned().unwrap_or_default();
            dependents_list.push(package_id.clone());
            dependents_guard.insert(dep.clone(), dependents_list);
        }

        // Clear resolution cache as it may be invalidated
        let cache_guard = self.resolution_cache.pin();
        cache_guard.clear();
    }

    pub fn remove_package(&self, package_id: &PackageId) {
        // Remove forward dependencies
        let deps_guard = self.dependencies.pin();
        if let Some(deps) = deps_guard.remove(package_id) {
            // Remove from reverse dependencies
            let dependents_guard = self.dependents.pin();
            for dep in deps {
                if let Some(mut dependents_list) = dependents_guard.get(dep).cloned() {
                    dependents_list.retain(|id| id != package_id);
                    dependents_guard.insert(dep.clone(), dependents_list);
                }
            }
        }

        // Remove as a dependent
        let dependents_guard = self.dependents.pin();
        dependents_guard.remove(package_id);

        // Clear resolution cache
        let cache_guard = self.resolution_cache.pin();
        cache_guard.clear();
    }

    pub fn get_dependencies(&self, package_id: &PackageId) -> Option<Vec<PackageId>> {
        let guard = self.dependencies.pin();
        guard.get(package_id).cloned()
    }

    pub fn get_dependents(&self, package_id: &PackageId) -> Option<Vec<PackageId>> {
        let guard = self.dependents.pin();
        guard.get(package_id).cloned()
    }

    pub fn resolve_install_order(&self, packages: &[PackageId]) -> Result<Vec<PackageId>> {
        // Check cache first
        let cache_key = packages.to_vec();
        let cache_guard = self.resolution_cache.pin();
        if let Some(cached) = cache_guard.get(&cache_key) {
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
        let cache_guard = self.resolution_cache.pin();
        cache_guard.insert(cache_key, result.clone());

        Ok(result)
    }

    pub fn clear(&self) {
        let deps_guard = self.dependencies.pin();
        deps_guard.clear();
        let dependents_guard = self.dependents.pin();
        dependents_guard.clear();
        let cache_guard = self.resolution_cache.pin();
        cache_guard.clear();
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
