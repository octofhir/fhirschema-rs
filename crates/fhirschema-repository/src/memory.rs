//! In-memory repository implementation for FHIRSchema storage

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use fhirschema_core::Schema;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::{
    repository::*,
    version::{SchemaVersion, VersionManager},
    RepositoryError, RepositoryResult,
};

/// In-memory repository implementation with thread-safe concurrent access
#[derive(Debug)]
pub struct MemoryRepository {
    /// Schema storage indexed by URL
    schemas: DashMap<String, SchemaEntry>,
    /// Repository metadata
    metadata: Arc<std::sync::RwLock<RepositoryMetadata>>,
    /// Configuration
    config: MemoryRepositoryConfig,
}

/// Schema entry in memory storage
#[derive(Debug, Clone)]
struct SchemaEntry {
    /// Current schema version
    current_schema: Schema,
    /// Version manager for this schema
    version_manager: VersionManager,
    /// All versions of this schema
    versions: HashMap<SchemaVersion, Schema>,
    /// Schema metadata
    metadata: SchemaMetadata,
}


/// Configuration for memory repository
#[derive(Debug, Clone)]
pub struct MemoryRepositoryConfig {
    /// Maximum number of schemas to store
    pub max_schemas: Option<usize>,
    /// Maximum storage size in bytes
    pub max_size: Option<u64>,
    /// Enable automatic cleanup of old versions
    pub auto_cleanup: bool,
    /// Maximum versions per schema
    pub max_versions_per_schema: Option<usize>,
}

impl Default for MemoryRepositoryConfig {
    fn default() -> Self {
        Self {
            max_schemas: None,
            max_size: None,
            auto_cleanup: false,
            max_versions_per_schema: Some(10),
        }
    }
}

impl MemoryRepository {
    /// Create a new memory repository
    pub fn new() -> Self {
        Self::with_config(MemoryRepositoryConfig::default())
    }

    /// Create a new memory repository with custom configuration
    pub fn with_config(config: MemoryRepositoryConfig) -> Self {
        let now = Utc::now();
        let metadata = RepositoryMetadata {
            created_at: now,
            last_updated: now,
            total_schemas: 0,
            total_versions: 0,
            total_size: 0,
        };

        Self {
            schemas: DashMap::new(),
            metadata: Arc::new(std::sync::RwLock::new(metadata)),
            config,
        }
    }

    /// Get current repository size in bytes (estimated)
    pub fn size(&self) -> u64 {
        self.metadata.read().unwrap().total_size
    }

    /// Get number of stored schemas
    pub fn schema_count(&self) -> usize {
        self.schemas.len()
    }

    /// Get total number of versions across all schemas
    pub fn version_count(&self) -> u64 {
        self.metadata.read().unwrap().total_versions
    }

    /// Check if repository has reached capacity limits
    fn check_capacity(&self) -> RepositoryResult<()> {
        if let Some(max_schemas) = self.config.max_schemas {
            if self.schemas.len() >= max_schemas {
                return Err(RepositoryError::generic(
                    format!("Repository has reached maximum schema limit: {}", max_schemas)
                ));
            }
        }

        if let Some(max_size) = self.config.max_size {
            if self.size() >= max_size {
                return Err(RepositoryError::QuotaExceeded {
                    current: self.size(),
                    limit: max_size,
                });
            }
        }

        Ok(())
    }

    /// Estimate schema size in bytes
    fn estimate_schema_size(schema: &Schema) -> u64 {
        // Rough estimation based on serialized JSON size
        serde_json::to_string(schema)
            .map(|s| s.len() as u64)
            .unwrap_or(1024) // Default estimate if serialization fails
    }

    /// Update repository metadata
    fn update_metadata(&self, size_delta: i64, version_delta: i64) {
        if let Ok(mut metadata) = self.metadata.write() {
            metadata.last_updated = Utc::now();
            metadata.total_size = (metadata.total_size as i64 + size_delta).max(0) as u64;
            metadata.total_versions = (metadata.total_versions as i64 + version_delta).max(0) as u64;
            metadata.total_schemas = self.schemas.len() as u64;
        }
    }

    /// Create schema metadata from schema
    fn create_metadata(schema: &Schema, version: &SchemaVersion) -> SchemaMetadata {
        let mut metadata = SchemaMetadata::new(schema.url.clone(), version.clone());

        metadata.name = Some(schema.name.clone());
        metadata.base = schema.base.clone();
        metadata.size = Self::estimate_schema_size(schema);

        // Determine schema type from URL or name
        metadata.schema_type = match schema.name.as_str() {
            n if n.ends_with("Extension") => SchemaType::Extension,
            n if n.starts_with("Profile") => SchemaType::Profile,
            _ => SchemaType::Resource,
        };

        // Set status to active by default
        metadata.status = SchemaStatus::Active;

        metadata
    }

    /// Cleanup old versions if auto cleanup is enabled
    fn cleanup_versions(&self, entry: &mut SchemaEntry) -> RepositoryResult<()> {
        if !self.config.auto_cleanup {
            return Ok(());
        }

        if let Some(max_versions) = self.config.max_versions_per_schema {
            let version_count = entry.versions.len();
            if version_count > max_versions {
                // Keep the most recent versions
                let mut versions: Vec<_> = entry.versions.keys().cloned().collect();
                versions.sort();
                versions.reverse(); // Most recent first

                let to_remove = versions.into_iter().skip(max_versions).collect::<Vec<_>>();
                let mut size_freed = 0u64;
                let remove_count = to_remove.len();

                for version in &to_remove {
                    if let Some(schema) = entry.versions.remove(version) {
                        size_freed += Self::estimate_schema_size(&schema);
                    }
                }

                self.update_metadata(-(size_freed as i64), -(remove_count as i64));
            }
        }

        Ok(())
    }
}

impl Default for MemoryRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchemaRepository for MemoryRepository {
    async fn store_schema(
        &self,
        schema: &Schema,
        metadata: Option<SchemaMetadata>,
    ) -> RepositoryResult<String> {
        self.check_capacity()?;

        let version = SchemaVersion::new(1, 0, 0); // Default version for new schemas
        let schema_size = Self::estimate_schema_size(schema);

        let schema_metadata = metadata.unwrap_or_else(|| Self::create_metadata(schema, &version));

        match self.schemas.get_mut(&schema.url) {
            Some(mut entry) => {
                // Update existing schema
                let current_version = entry.version_manager.current();
                let new_version = current_version.next_minor();

                entry.version_manager.add_version(new_version.clone())?;

                entry.versions.insert(new_version.clone(), schema.clone());
                entry.current_schema = schema.clone();
                entry.metadata = schema_metadata;
                entry.metadata.version = new_version;
                entry.metadata.touch();

                self.cleanup_versions(&mut entry)?;
                // Drop the entry reference before calling update_metadata to avoid deadlock
                drop(entry);
                self.update_metadata(schema_size as i64, 1);
            }
            None => {
                // Create new schema entry
                let version_manager = VersionManager::new(version.clone());
                let mut versions = HashMap::new();
                versions.insert(version.clone(), schema.clone());

                let entry = SchemaEntry {
                    current_schema: schema.clone(),
                    version_manager,
                    versions,
                    metadata: schema_metadata,
                };

                self.schemas.insert(schema.url.clone(), entry);
                self.update_metadata(schema_size as i64, 1);
            }
        }

        Ok(schema.url.clone())
    }

    async fn get_schema(&self, url: &str) -> RepositoryResult<Schema> {
        self.get_latest_schema(url).await
    }

    async fn get_schema_version(
        &self,
        url: &str,
        version: &SchemaVersion,
    ) -> RepositoryResult<Schema> {
        let entry = self.schemas.get(url)
            .ok_or_else(|| RepositoryError::schema_not_found(url))?;

        entry.versions.get(version)
            .cloned()
            .ok_or_else(|| RepositoryError::version_not_found(url, version.to_string()))
    }

    async fn get_latest_schema(&self, url: &str) -> RepositoryResult<Schema> {
        let entry = self.schemas.get(url)
            .ok_or_else(|| RepositoryError::schema_not_found(url))?;

        Ok(entry.current_schema.clone())
    }

    async fn list_schemas(&self, query: &SchemaQuery) -> RepositoryResult<Vec<SchemaMetadata>> {
        let mut results = Vec::new();

        for entry in self.schemas.iter() {
            let metadata = &entry.metadata;

            // Apply filters
            if let Some(ref schema_type) = query.schema_type {
                if metadata.schema_type != *schema_type {
                    continue;
                }
            }

            if let Some(ref status) = query.status {
                if metadata.status != *status {
                    continue;
                }
            }

            if let Some(ref base) = query.base {
                if metadata.base.as_ref() != Some(base) {
                    continue;
                }
            }

            if !query.tags.is_empty() {
                if !query.tags.iter().any(|tag| metadata.tags.contains(tag)) {
                    continue;
                }
            }

            if let Some(ref text) = query.text {
                let text_lower = text.to_lowercase();
                let schema_type_str = format!("{:?}", metadata.schema_type).to_lowercase();
                let matches = metadata.name.as_ref().map_or(false, |n| n.to_lowercase().contains(&text_lower))
                    || metadata.title.as_ref().map_or(false, |t| t.to_lowercase().contains(&text_lower))
                    || metadata.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&text_lower))
                    || schema_type_str.contains(&text_lower);

                if !matches {
                    continue;
                }
            }

            results.push(metadata.clone());
        }

        // Apply sorting
        if let Some(ref sort) = query.sort {
            match sort {
                SortOrder::NameAsc => results.sort_by(|a, b| a.name.cmp(&b.name)),
                SortOrder::NameDesc => results.sort_by(|a, b| b.name.cmp(&a.name)),
                SortOrder::VersionAsc => results.sort_by(|a, b| a.version.cmp(&b.version)),
                SortOrder::VersionDesc => results.sort_by(|a, b| b.version.cmp(&a.version)),
                SortOrder::CreatedAsc => results.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
                SortOrder::CreatedDesc => results.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
                SortOrder::UpdatedAsc => results.sort_by(|a, b| a.updated_at.cmp(&b.updated_at)),
                SortOrder::UpdatedDesc => results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
            }
        }

        // Apply pagination
        if let Some(offset) = query.offset {
            if offset < results.len() {
                results = results.into_iter().skip(offset).collect();
            } else {
                results.clear();
            }
        }

        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn schema_exists(&self, url: &str) -> RepositoryResult<bool> {
        Ok(self.schemas.contains_key(url))
    }

    async fn remove_schema(&self, url: &str) -> RepositoryResult<()> {
        let (_, entry) = self.schemas.remove(url)
            .ok_or_else(|| RepositoryError::schema_not_found(url))?;

        let size_freed = entry.versions.values()
            .map(Self::estimate_schema_size)
            .sum::<u64>();
        let versions_removed = entry.versions.len();

        self.update_metadata(-(size_freed as i64), -(versions_removed as i64));

        Ok(())
    }

    async fn remove_schema_version(
        &self,
        url: &str,
        version: &SchemaVersion,
    ) -> RepositoryResult<()> {
        let mut entry = self.schemas.get_mut(url)
            .ok_or_else(|| RepositoryError::schema_not_found(url))?;

        let schema = entry.versions.remove(version)
            .ok_or_else(|| RepositoryError::version_not_found(url, version.to_string()))?;

        entry.version_manager.remove_version(version)?;

        // Update current schema if we removed the current version
        if &entry.metadata.version == version {
            if let Some(latest_version) = entry.version_manager.history().iter().max().cloned() {
                if let Some(latest_schema) = entry.versions.get(&latest_version) {
                    entry.current_schema = latest_schema.clone();
                    entry.metadata.version = latest_version;
                    entry.metadata.touch();
                }
            }
        }

        let size_freed = Self::estimate_schema_size(&schema);
        self.update_metadata(-(size_freed as i64), -1);

        Ok(())
    }

    async fn get_metadata(&self, url: &str) -> RepositoryResult<SchemaMetadata> {
        let entry = self.schemas.get(url)
            .ok_or_else(|| RepositoryError::schema_not_found(url))?;

        Ok(entry.metadata.clone())
    }

    async fn update_metadata(
        &self,
        url: &str,
        metadata: &SchemaMetadata,
    ) -> RepositoryResult<()> {
        let mut entry = self.schemas.get_mut(url)
            .ok_or_else(|| RepositoryError::schema_not_found(url))?;

        entry.metadata = metadata.clone();
        entry.metadata.touch();

        Ok(())
    }

    async fn search_schemas(&self, query: &str) -> RepositoryResult<Vec<SchemaMetadata>> {
        let search_query = SchemaQuery {
            text: Some(query.to_string()),
            ..Default::default()
        };

        self.list_schemas(&search_query).await
    }

    async fn get_statistics(&self) -> RepositoryResult<RepositoryStatistics> {
        let metadata = self.metadata.read().unwrap();
        let mut schemas_by_type = HashMap::new();
        let mut schemas_by_status = HashMap::new();

        for entry in self.schemas.iter() {
            let schema_metadata = &entry.metadata;
            *schemas_by_type.entry(schema_metadata.schema_type.clone()).or_insert(0) += 1;
            *schemas_by_status.entry(schema_metadata.status.clone()).or_insert(0) += 1;
        }

        Ok(RepositoryStatistics {
            total_schemas: metadata.total_schemas,
            total_versions: metadata.total_versions,
            total_size: metadata.total_size,
            schemas_by_type,
            schemas_by_status,
            created_at: metadata.created_at,
            last_updated: metadata.last_updated,
        })
    }

    async fn validate_integrity(&self) -> RepositoryResult<IntegrityReport> {
        let start_time = Instant::now();
        let mut issues = Vec::new();
        let mut schemas_checked = 0u64;

        for entry in self.schemas.iter() {
            schemas_checked += 1;
            let url = entry.key();
            let schema_entry = entry.value();

            // Check if current schema exists in versions
            if !schema_entry.versions.contains_key(&schema_entry.metadata.version) {
                issues.push(IntegrityIssue {
                    severity: IssueSeverity::Error,
                    schema_url: url.clone(),
                    description: "Current schema version not found in versions map".to_string(),
                    suggested_fix: Some("Re-store the schema to fix version mapping".to_string()),
                });
            }

            // Check version manager consistency
            for version in schema_entry.version_manager.history() {
                if !schema_entry.versions.contains_key(version) {
                    issues.push(IntegrityIssue {
                        severity: IssueSeverity::Warning,
                        schema_url: url.clone(),
                        description: format!("Version {} in history but not in storage", version),
                        suggested_fix: Some("Clean up version history".to_string()),
                    });
                }
            }

            // Check for orphaned versions
            for version in schema_entry.versions.keys() {
                if !schema_entry.version_manager.has_version(version) {
                    issues.push(IntegrityIssue {
                        severity: IssueSeverity::Info,
                        schema_url: url.clone(),
                        description: format!("Version {} stored but not in history", version),
                        suggested_fix: Some("Add version to history or remove from storage".to_string()),
                    });
                }
            }
        }

        let is_valid = issues.iter().all(|issue| matches!(issue.severity, IssueSeverity::Info | IssueSeverity::Warning));

        Ok(IntegrityReport {
            is_valid,
            issues,
            schemas_checked,
            check_duration: start_time.elapsed(),
        })
    }

    async fn cleanup(&self, options: &CleanupOptions) -> RepositoryResult<CleanupReport> {
        let start_time = Instant::now();
        let mut schemas_removed = 0u64;
        let mut versions_removed = 0u64;
        let mut space_freed = 0u64;
        let mut removed_schemas = Vec::new();

        if options.dry_run {
            // For dry run, just calculate what would be removed
            for entry in self.schemas.iter() {
                let url = entry.key();
                let schema_entry = entry.value();
                let metadata = &schema_entry.metadata;

                let should_remove = if let Some(max_age) = options.max_age {
                    metadata.updated_at < Utc::now() - chrono::Duration::from_std(max_age).unwrap()
                } else {
                    false
                };

                let is_unused = options.remove_unused && !metadata.has_dependents();

                let is_old_draft = if let Some(draft_max_age) = options.draft_max_age {
                    metadata.status == SchemaStatus::Draft &&
                    metadata.updated_at < Utc::now() - chrono::Duration::from_std(draft_max_age).unwrap()
                } else {
                    false
                };

                if should_remove || is_unused || is_old_draft {
                    schemas_removed += 1;
                    versions_removed += schema_entry.versions.len() as u64;
                    space_freed += schema_entry.versions.values()
                        .map(Self::estimate_schema_size)
                        .sum::<u64>();
                    removed_schemas.push(url.clone());
                }
            }
        } else {
            // Actually remove schemas
            let urls_to_remove: Vec<String> = self.schemas.iter()
                .filter_map(|entry| {
                    let url = entry.key();
                    let schema_entry = entry.value();
                    let metadata = &schema_entry.metadata;

                    let should_remove = if let Some(max_age) = options.max_age {
                        metadata.updated_at < Utc::now() - chrono::Duration::from_std(max_age).unwrap()
                    } else {
                        false
                    };

                    let is_unused = options.remove_unused && !metadata.has_dependents();

                    let is_old_draft = if let Some(draft_max_age) = options.draft_max_age {
                        metadata.status == SchemaStatus::Draft &&
                        metadata.updated_at < Utc::now() - chrono::Duration::from_std(draft_max_age).unwrap()
                    } else {
                        false
                    };

                    if should_remove || is_unused || is_old_draft {
                        Some(url.clone())
                    } else {
                        None
                    }
                })
                .collect();

            for url in urls_to_remove {
                if let Some((_, entry)) = self.schemas.remove(&url) {
                    schemas_removed += 1;
                    versions_removed += entry.versions.len() as u64;
                    space_freed += entry.versions.values()
                        .map(Self::estimate_schema_size)
                        .sum::<u64>();
                    removed_schemas.push(url);
                }
            }

            self.update_metadata(-(space_freed as i64), -(versions_removed as i64));
        }

        Ok(CleanupReport {
            schemas_removed,
            versions_removed,
            space_freed,
            removed_schemas,
            duration: start_time.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fhirschema_core::Schema;

    #[tokio::test]
    async fn test_store_and_retrieve_schema() {
        let repo = MemoryRepository::new();

        let schema = Schema::new(
            "http://example.com/test".to_string(),
            "Resource".to_string(),
            "Test".to_string(),
            "specialization".to_string(),
        );

        // Store schema
        let stored_url = repo.store_schema(&schema, None).await.unwrap();
        assert_eq!(stored_url, "http://example.com/test");

        // Retrieve schema
        let retrieved = repo.get_schema("http://example.com/test").await.unwrap();
        assert_eq!(retrieved.url, "http://example.com/test");
        assert_eq!(retrieved.name, "Test");
    }

    #[tokio::test]
    async fn test_schema_versioning() {
        let repo = MemoryRepository::new();

        let schema_v1 = Schema::new(
            "http://example.com/versioned".to_string(),
            "Resource".to_string(),
            "Test V1".to_string(),
            "specialization".to_string(),
        );

        let schema_v2 = Schema::new(
            "http://example.com/versioned".to_string(),
            "Resource".to_string(),
            "Test V2".to_string(),
            "specialization".to_string(),
        );

        // Store first version
        repo.store_schema(&schema_v1, None).await.unwrap();

        // Store second version
        repo.store_schema(&schema_v2, None).await.unwrap();

        // Latest should be V2
        let latest = repo.get_latest_schema("http://example.com/versioned").await.unwrap();
        assert_eq!(latest.name, "Test V2");

        // Should have 2 versions
        assert_eq!(repo.version_count(), 2);
    }

    #[tokio::test]
    async fn test_schema_search() {
        let repo = MemoryRepository::new();

        let schema1 = Schema::new(
            "http://example.com/patient".to_string(),
            "Resource".to_string(),
            "Patient".to_string(),
            "specialization".to_string(),
        );

        let schema2 = Schema::new(
            "http://example.com/observation".to_string(),
            "Resource".to_string(),
            "Observation".to_string(),
            "specialization".to_string(),
        );

        repo.store_schema(&schema1, None).await.unwrap();
        repo.store_schema(&schema2, None).await.unwrap();

        // Search for "Patient"
        let results = repo.search_schemas("Patient").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, Some("Patient".to_string()));

        // Search for "Resource"
        let results = repo.search_schemas("Resource").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_repository_statistics() {
        let repo = MemoryRepository::new();

        let schema = Schema::new(
            "http://example.com/stats".to_string(),
            "Resource".to_string(),
            "Stats".to_string(),
            "specialization".to_string(),
        );

        repo.store_schema(&schema, None).await.unwrap();

        let stats = repo.get_statistics().await.unwrap();
        assert_eq!(stats.total_schemas, 1);
        assert_eq!(stats.total_versions, 1);
        assert!(stats.total_size > 0);
    }
}
