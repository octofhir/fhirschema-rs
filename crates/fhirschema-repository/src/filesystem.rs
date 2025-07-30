//! File system repository implementation for FHIRSchema storage

use async_trait::async_trait;
use chrono::Utc;
use fhirschema_core::Schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use walkdir::WalkDir;

use crate::{
    repository::*,
    version::{SchemaVersion, VersionManager},
    RepositoryError, RepositoryResult,
};

/// File system repository implementation with persistent storage
#[derive(Debug)]
pub struct FileSystemRepository {
    /// Root directory for schema storage
    root_path: PathBuf,
    /// Repository metadata
    metadata: Arc<RwLock<RepositoryMetadata>>,
    /// Configuration
    config: FileSystemRepositoryConfig,
    /// Index for fast lookups
    index: Arc<RwLock<SchemaIndex>>,
}

/// Configuration for file system repository
#[derive(Debug, Clone)]
pub struct FileSystemRepositoryConfig {
    /// Enable automatic indexing
    pub auto_index: bool,
    /// Enable atomic operations
    pub atomic_operations: bool,
    /// Maximum file size in bytes
    pub max_file_size: u64,
    /// Enable compression
    pub enable_compression: bool,
    /// Backup retention count
    pub backup_retention: usize,
}

impl Default for FileSystemRepositoryConfig {
    fn default() -> Self {
        Self {
            auto_index: true,
            atomic_operations: true,
            max_file_size: 10 * 1024 * 1024, // 10MB
            enable_compression: false,
            backup_retention: 5,
        }
    }
}

/// Schema index for fast lookups
#[derive(Debug, Default, Serialize, Deserialize)]
struct SchemaIndex {
    /// Map from URL to file path
    url_to_path: HashMap<String, PathBuf>,
    /// Map from name to URLs
    name_to_urls: HashMap<String, Vec<String>>,
    /// Schema metadata cache
    metadata_cache: HashMap<String, SchemaMetadata>,
    /// Last update timestamp
    last_updated: chrono::DateTime<Utc>,
}

/// Schema file entry
#[derive(Debug, Serialize, Deserialize)]
struct SchemaFileEntry {
    /// Schema data
    schema: Schema,
    /// Version manager
    version_manager: VersionManager,
    /// All versions (stored as Vec for JSON compatibility)
    #[serde(with = "version_map_serde")]
    versions: HashMap<SchemaVersion, Schema>,
    /// Metadata
    metadata: SchemaMetadata,
    /// File format version
    format_version: u32,
}

/// Custom serialization for HashMap<SchemaVersion, Schema>
mod version_map_serde {
    use super::*;
    use serde::{Deserializer, Serializer, Deserialize};

    pub fn serialize<S>(
        map: &HashMap<SchemaVersion, Schema>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<(String, &Schema)> = map
            .iter()
            .map(|(version, schema)| (version.to_string(), schema))
            .collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<SchemaVersion, Schema>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<(String, Schema)> = Vec::deserialize(deserializer)?;
        let mut map = HashMap::new();
        for (version_str, schema) in vec {
            let version = SchemaVersion::parse(&version_str)
                .map_err(|e| serde::de::Error::custom(format!("Invalid version: {}", e)))?;
            map.insert(version, schema);
        }
        Ok(map)
    }
}

impl FileSystemRepository {
    /// Create a new file system repository
    pub async fn new<P: AsRef<Path>>(root_path: P) -> RepositoryResult<Self> {
        Self::with_config(root_path, FileSystemRepositoryConfig::default()).await
    }

    /// Create a new file system repository with custom configuration
    pub async fn with_config<P: AsRef<Path>>(
        root_path: P,
        config: FileSystemRepositoryConfig,
    ) -> RepositoryResult<Self> {
        let root_path = root_path.as_ref().to_path_buf();

        // Create root directory if it doesn't exist
        if !root_path.exists() {
            fs::create_dir_all(&root_path).await
                .map_err(|e| RepositoryError::generic(format!("Failed to create repository directory: {}", e)))?;
        }

        let metadata = Arc::new(RwLock::new(RepositoryMetadata {
            created_at: Utc::now(),
            last_updated: Utc::now(),
            total_schemas: 0,
            total_versions: 0,
            total_size: 0,
        }));

        let index = Arc::new(RwLock::new(SchemaIndex::default()));

        let repo = Self {
            root_path,
            metadata,
            config,
            index,
        };

        // Load existing index if available
        repo.load_index().await?;

        // Rebuild index if auto-indexing is enabled
        if repo.config.auto_index {
            repo.rebuild_index().await?;
        } else {
            // Save empty index if it doesn't exist
            repo.save_index().await?;
        }

        Ok(repo)
    }

    /// Get the file path for a schema URL
    fn get_schema_path(&self, url: &str) -> PathBuf {
        // Create a safe filename from the URL
        let safe_name = url
            .replace("://", "_")
            .replace("/", "_")
            .replace(":", "_")
            .replace("?", "_")
            .replace("#", "_");

        self.root_path.join("schemas").join(format!("{}.json", safe_name))
    }

    /// Get the index file path
    fn get_index_path(&self) -> PathBuf {
        self.root_path.join("index.json")
    }

    /// Load the index from disk
    async fn load_index(&self) -> RepositoryResult<()> {
        let index_path = self.get_index_path();

        if index_path.exists() {
            let content = fs::read_to_string(&index_path).await
                .map_err(|e| RepositoryError::generic(format!("Failed to read index: {}", e)))?;

            let loaded_index: SchemaIndex = serde_json::from_str(&content)
                .map_err(|e| RepositoryError::invalid_schema(format!("Invalid index format: {}", e)))?;

            *self.index.write().await = loaded_index;
        }

        Ok(())
    }

    /// Save the index to disk
    async fn save_index(&self) -> RepositoryResult<()> {
        let index_path = self.get_index_path();
        let index = self.index.read().await;

        let content = serde_json::to_string_pretty(&*index)
            .map_err(|e| RepositoryError::invalid_schema(format!("Failed to serialize index: {}", e)))?;

        if self.config.atomic_operations {
            let temp_path = index_path.with_extension("tmp");
            fs::write(&temp_path, content).await
                .map_err(|e| RepositoryError::generic(format!("Failed to write index: {}", e)))?;

            fs::rename(&temp_path, &index_path).await
                .map_err(|e| RepositoryError::generic(format!("Failed to rename index: {}", e)))?;
        } else {
            fs::write(&index_path, content).await
                .map_err(|e| RepositoryError::generic(format!("Failed to write index: {}", e)))?;
        }

        Ok(())
    }

    /// Rebuild the index by scanning all schema files
    async fn rebuild_index(&self) -> RepositoryResult<()> {
        let schemas_dir = self.root_path.join("schemas");

        if !schemas_dir.exists() {
            fs::create_dir_all(&schemas_dir).await
                .map_err(|e| RepositoryError::generic(format!("Failed to create schemas directory: {}", e)))?;
            return Ok(());
        }

        let mut new_index = SchemaIndex::default();
        let mut total_schemas = 0u64;
        let mut total_versions = 0u64;
        let mut total_size = 0u64;

        for entry in WalkDir::new(&schemas_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() && entry.path().extension().map_or(false, |ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(entry.path()).await {
                    if let Ok(schema_entry) = serde_json::from_str::<SchemaFileEntry>(&content) {
                        let url = &schema_entry.schema.url;
                        let name = &schema_entry.schema.name;

                        new_index.url_to_path.insert(url.clone(), entry.path().to_path_buf());
                        new_index.name_to_urls.entry(name.clone()).or_default().push(url.clone());
                        new_index.metadata_cache.insert(url.clone(), schema_entry.metadata);

                        total_schemas += 1;
                        total_versions += schema_entry.versions.len() as u64;
                        total_size += content.len() as u64;
                    }
                }
            }
        }

        new_index.last_updated = Utc::now();
        *self.index.write().await = new_index;

        // Update repository metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.total_schemas = total_schemas;
            metadata.total_versions = total_versions;
            metadata.total_size = total_size;
            metadata.last_updated = Utc::now();
        }

        self.save_index().await?;
        Ok(())
    }

    /// Load a schema file entry from disk
    async fn load_schema_entry(&self, url: &str) -> RepositoryResult<SchemaFileEntry> {
        let index = self.index.read().await;
        let path = index.url_to_path.get(url)
            .ok_or_else(|| RepositoryError::schema_not_found(url))?;

        let content = fs::read_to_string(path).await
            .map_err(|e| RepositoryError::generic(format!("Failed to read schema file: {}", e)))?;

        let entry: SchemaFileEntry = serde_json::from_str(&content)
            .map_err(|e| RepositoryError::invalid_schema(format!("Invalid schema file format: {}", e)))?;

        Ok(entry)
    }

    /// Save a schema file entry to disk
    async fn save_schema_entry(&self, url: &str, entry: &SchemaFileEntry) -> RepositoryResult<()> {
        let path = self.get_schema_path(url);

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| RepositoryError::generic(format!("Failed to create directory: {}", e)))?;
        }

        let content = serde_json::to_string_pretty(entry)
            .map_err(|e| RepositoryError::invalid_schema(format!("Failed to serialize schema: {}", e)))?;

        // Check file size limit
        if content.len() as u64 > self.config.max_file_size {
            return Err(RepositoryError::invalid_schema(
                format!("Schema file too large: {} bytes", content.len())
            ));
        }

        if self.config.atomic_operations {
            let temp_path = path.with_extension("tmp");
            fs::write(&temp_path, &content).await
                .map_err(|e| RepositoryError::generic(format!("Failed to write schema file: {}", e)))?;

            fs::rename(&temp_path, &path).await
                .map_err(|e| RepositoryError::generic(format!("Failed to rename schema file: {}", e)))?;
        } else {
            fs::write(&path, &content).await
                .map_err(|e| RepositoryError::generic(format!("Failed to write schema file: {}", e)))?;
        }

        // Update index
        {
            let mut index = self.index.write().await;
            index.url_to_path.insert(url.to_string(), path);
            index.name_to_urls.entry(entry.schema.name.clone()).or_default().push(url.to_string());
            index.metadata_cache.insert(url.to_string(), entry.metadata.clone());
            index.last_updated = Utc::now();
        }

        self.save_index().await?;
        Ok(())
    }

    /// Remove a schema file from disk
    async fn remove_schema_file(&self, url: &str) -> RepositoryResult<()> {
        let index = self.index.read().await;
        if let Some(path) = index.url_to_path.get(url) {
            if path.exists() {
                fs::remove_file(path).await
                    .map_err(|e| RepositoryError::generic(format!("Failed to remove schema file: {}", e)))?;
            }
        }
        drop(index);

        // Update index
        {
            let mut index = self.index.write().await;
            index.url_to_path.remove(url);

            // Remove from name mapping
            for urls in index.name_to_urls.values_mut() {
                urls.retain(|u| u != url);
            }
            index.name_to_urls.retain(|_, urls| !urls.is_empty());

            index.metadata_cache.remove(url);
            index.last_updated = Utc::now();
        }

        self.save_index().await?;
        Ok(())
    }
}

#[async_trait]
impl SchemaRepository for FileSystemRepository {
    async fn store_schema(
        &self,
        schema: &Schema,
        metadata: Option<SchemaMetadata>,
    ) -> RepositoryResult<String> {
        let url = &schema.url;

        // Try to load existing entry
        let mut entry = match self.load_schema_entry(url).await {
            Ok(mut existing) => {
                // Update existing schema
                let current_version = existing.version_manager.current();
                let new_version = current_version.next_minor();

                existing.version_manager.add_version(new_version.clone())?;
                existing.versions.insert(new_version.clone(), schema.clone());
                existing.schema = schema.clone();
                existing.metadata = metadata.unwrap_or_else(|| {
                    let mut meta = SchemaMetadata::new(url.clone(), new_version.clone());
                    meta.name = Some(schema.name.clone());
                    meta.base = schema.base.clone();
                    meta.touch();
                    meta
                });
                existing.metadata.version = new_version;
                existing.metadata.touch();

                existing
            }
            Err(_) => {
                // Create new entry
                let version = SchemaVersion::new(1, 0, 0);
                let version_manager = VersionManager::new(version.clone());
                let mut versions = HashMap::new();
                versions.insert(version.clone(), schema.clone());

                let schema_metadata = metadata.unwrap_or_else(|| {
                    let mut meta = SchemaMetadata::new(url.clone(), version.clone());
                    meta.name = Some(schema.name.clone());
                    meta.base = schema.base.clone();
                    meta.touch();
                    meta
                });

                SchemaFileEntry {
                    schema: schema.clone(),
                    version_manager,
                    versions,
                    metadata: schema_metadata,
                    format_version: 1,
                }
            }
        };

        self.save_schema_entry(url, &entry).await?;

        // Update repository metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.last_updated = Utc::now();
            // Metadata will be updated during index rebuild
        }

        Ok(url.clone())
    }

    async fn get_schema(&self, url: &str) -> RepositoryResult<Schema> {
        let entry = self.load_schema_entry(url).await?;
        Ok(entry.schema)
    }

    async fn get_schema_version(
        &self,
        url: &str,
        version: &SchemaVersion,
    ) -> RepositoryResult<Schema> {
        let entry = self.load_schema_entry(url).await?;
        entry.versions.get(version)
            .cloned()
            .ok_or_else(|| RepositoryError::version_not_found(url, version.to_string()))
    }

    async fn get_latest_schema(&self, url: &str) -> RepositoryResult<Schema> {
        self.get_schema(url).await
    }

    async fn list_schemas(&self, query: &SchemaQuery) -> RepositoryResult<Vec<SchemaMetadata>> {
        let index = self.index.read().await;
        let mut results = Vec::new();

        for metadata in index.metadata_cache.values() {
            // Apply filters (same logic as MemoryRepository)
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

        // Apply sorting and pagination (same logic as MemoryRepository)
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
        let index = self.index.read().await;
        Ok(index.url_to_path.contains_key(url))
    }

    async fn remove_schema(&self, url: &str) -> RepositoryResult<()> {
        self.remove_schema_file(url).await?;

        // Update repository metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.last_updated = Utc::now();
        }

        Ok(())
    }

    async fn remove_schema_version(
        &self,
        url: &str,
        version: &SchemaVersion,
    ) -> RepositoryResult<()> {
        let mut entry = self.load_schema_entry(url).await?;

        entry.versions.remove(version)
            .ok_or_else(|| RepositoryError::version_not_found(url, version.to_string()))?;

        entry.version_manager.remove_version(version)?;

        // Update current schema if we removed the current version
        if &entry.metadata.version == version {
            if let Some(latest_version) = entry.version_manager.history().iter().max().cloned() {
                if let Some(latest_schema) = entry.versions.get(&latest_version) {
                    entry.schema = latest_schema.clone();
                    entry.metadata.version = latest_version;
                    entry.metadata.touch();
                }
            }
        }

        self.save_schema_entry(url, &entry).await?;
        Ok(())
    }

    async fn get_metadata(&self, url: &str) -> RepositoryResult<SchemaMetadata> {
        let index = self.index.read().await;
        index.metadata_cache.get(url)
            .cloned()
            .ok_or_else(|| RepositoryError::schema_not_found(url))
    }

    async fn update_metadata(
        &self,
        url: &str,
        metadata: &SchemaMetadata,
    ) -> RepositoryResult<()> {
        let mut entry = self.load_schema_entry(url).await?;
        entry.metadata = metadata.clone();
        entry.metadata.touch();

        self.save_schema_entry(url, &entry).await?;
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
        let metadata = self.metadata.read().await;
        let index = self.index.read().await;

        let mut schemas_by_type = HashMap::new();
        let mut schemas_by_status = HashMap::new();

        for schema_metadata in index.metadata_cache.values() {
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
        let start_time = std::time::Instant::now();
        let mut issues = Vec::new();
        let mut schemas_checked = 0u64;

        let index = self.index.read().await;

        for (url, path) in &index.url_to_path {
            schemas_checked += 1;

            // Check if file exists
            if !path.exists() {
                issues.push(IntegrityIssue {
                    severity: IssueSeverity::Error,
                    schema_url: url.clone(),
                    description: "Schema file not found on disk".to_string(),
                    suggested_fix: Some("Remove from index or restore file".to_string()),
                });
                continue;
            }

            // Try to load and validate the schema file
            match fs::read_to_string(path).await {
                Ok(content) => {
                    match serde_json::from_str::<SchemaFileEntry>(&content) {
                        Ok(entry) => {
                            // Validate version consistency
                            for version in entry.version_manager.history() {
                                if !entry.versions.contains_key(version) {
                                    issues.push(IntegrityIssue {
                                        severity: IssueSeverity::Warning,
                                        schema_url: url.clone(),
                                        description: format!("Version {} in history but not in storage", version),
                                        suggested_fix: Some("Clean up version history".to_string()),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            issues.push(IntegrityIssue {
                                severity: IssueSeverity::Error,
                                schema_url: url.clone(),
                                description: format!("Invalid schema file format: {}", e),
                                suggested_fix: Some("Fix file format or remove file".to_string()),
                            });
                        }
                    }
                }
                Err(e) => {
                    issues.push(IntegrityIssue {
                        severity: IssueSeverity::Error,
                        schema_url: url.clone(),
                        description: format!("Cannot read schema file: {}", e),
                        suggested_fix: Some("Check file permissions".to_string()),
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
        let start_time = std::time::Instant::now();
        let mut schemas_removed = 0u64;
        let mut versions_removed = 0u64;
        let mut space_freed = 0u64;
        let mut removed_schemas = Vec::new();

        if options.dry_run {
            // For dry run, just calculate what would be removed
            let index = self.index.read().await;
            for (url, path) in &index.url_to_path {
                if let Some(metadata) = index.metadata_cache.get(url) {
                    let should_remove = if let Some(max_age) = options.max_age {
                        metadata.updated_at < Utc::now() - chrono::Duration::from_std(max_age).unwrap()
                    } else {
                        false
                    };

                    if should_remove {
                        schemas_removed += 1;
                        if let Ok(file_metadata) = std::fs::metadata(path) {
                            space_freed += file_metadata.len();
                        }
                        removed_schemas.push(url.clone());
                    }
                }
            }
        } else {
            // Actually remove schemas
            let urls_to_remove: Vec<String> = {
                let index = self.index.read().await;
                index.metadata_cache.iter()
                    .filter_map(|(url, metadata)| {
                        let should_remove = if let Some(max_age) = options.max_age {
                            metadata.updated_at < Utc::now() - chrono::Duration::from_std(max_age).unwrap()
                        } else {
                            false
                        };

                        if should_remove {
                            Some(url.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            for url in urls_to_remove {
                if let Ok(path) = {
                    let index = self.index.read().await;
                    index.url_to_path.get(&url).cloned()
                        .ok_or_else(|| RepositoryError::schema_not_found(&url))
                } {
                    if let Ok(file_metadata) = std::fs::metadata(&path) {
                        space_freed += file_metadata.len();
                    }

                    self.remove_schema_file(&url).await?;
                    schemas_removed += 1;
                    removed_schemas.push(url);
                }
            }
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
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_filesystem_repository_creation() {
        let temp_dir = TempDir::new().unwrap();
        let repo = FileSystemRepository::new(temp_dir.path()).await.unwrap();

        assert!(temp_dir.path().join("schemas").exists());
        assert!(temp_dir.path().join("index.json").exists());
    }

    #[tokio::test]
    async fn test_store_and_retrieve_schema() {
        let temp_dir = TempDir::new().unwrap();
        let repo = FileSystemRepository::new(temp_dir.path()).await.unwrap();

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
    async fn test_schema_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create repository and store schema
        {
            let repo = FileSystemRepository::new(temp_dir.path()).await.unwrap();
            let schema = Schema::new(
                "http://example.com/persistent".to_string(),
                "Resource".to_string(),
                "Persistent".to_string(),
                "specialization".to_string(),
            );
            repo.store_schema(&schema, None).await.unwrap();
        }

        // Create new repository instance and verify schema persists
        {
            let repo = FileSystemRepository::new(temp_dir.path()).await.unwrap();
            let retrieved = repo.get_schema("http://example.com/persistent").await.unwrap();
            assert_eq!(retrieved.name, "Persistent");
        }
    }
}
