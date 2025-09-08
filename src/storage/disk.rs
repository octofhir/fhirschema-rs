use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::{FhirSchemaError, Result};
use crate::storage::SchemaStorage;
use crate::types::FhirSchema;
use crate::utils::{PackageFingerprint, fingerprint::generate_package_fingerprint};

/// Disk-based storage with fingerprinting and caching
#[derive(Debug)]
pub struct DiskStorage {
    /// Root directory for storage
    cache_dir: PathBuf,
    /// In-memory index for fast lookups
    index: HashMap<String, CacheEntry>,
    /// Storage configuration  
    config: DiskStorageConfig,
}

#[derive(Debug, Clone)]
pub struct DiskStorageConfig {
    /// Enable compression for stored schemas
    pub enable_compression: bool,
    /// Maximum size of cache directory in bytes
    pub max_cache_size: Option<u64>,
    /// Use binary serialization (faster) vs JSON (human readable)
    pub binary_serialization: bool,
    /// Automatically clean old cache entries
    pub auto_cleanup: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    /// Schema URL
    url: String,
    /// File path relative to cache_dir
    file_path: PathBuf,
    /// Package fingerprint for validation
    fingerprint: PackageFingerprint,
    /// Last access time
    last_accessed: chrono::DateTime<chrono::Utc>,
    /// File size in bytes
    file_size: u64,
}

impl Default for DiskStorageConfig {
    fn default() -> Self {
        Self {
            enable_compression: true,
            max_cache_size: Some(1024 * 1024 * 1024), // 1GB
            binary_serialization: cfg!(feature = "embedded-providers"),
            auto_cleanup: true,
        }
    }
}

impl DiskStorage {
    /// Create a new disk storage instance
    pub async fn new(cache_dir: impl AsRef<Path>) -> Result<Self> {
        Self::with_config(cache_dir, DiskStorageConfig::default()).await
    }

    /// Create disk storage with custom configuration
    pub async fn with_config(
        cache_dir: impl AsRef<Path>,
        config: DiskStorageConfig,
    ) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        // Create cache directory if it doesn't exist
        fs::create_dir_all(&cache_dir).await.map_err(|e| {
            FhirSchemaError::io_error(&format!("Failed to create cache directory: {e}"))
        })?;

        let mut storage = Self {
            cache_dir,
            index: HashMap::new(),
            config,
        };

        // Load existing index
        storage.load_index().await?;

        Ok(storage)
    }

    /// Get default cache directory
    pub fn default_cache_dir() -> Result<PathBuf> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            FhirSchemaError::configuration_error("Unable to determine home directory")
        })?;

        let cache_dir = home_dir.join(".fhir").join(".fhirschema");

        Ok(cache_dir)
    }

    /// Load index from disk
    async fn load_index(&mut self) -> Result<()> {
        let index_path = self.cache_dir.join("index.json");

        if !index_path.exists() {
            // No existing index, start fresh
            return Ok(());
        }

        let index_data = fs::read_to_string(&index_path)
            .await
            .map_err(|e| FhirSchemaError::io_error(&format!("Failed to read index: {e}")))?;

        let entries: Vec<CacheEntry> = serde_json::from_str(&index_data).map_err(|e| {
            FhirSchemaError::serialization_error(&format!("Failed to parse index: {e}"))
        })?;

        // Rebuild index HashMap
        for entry in entries {
            self.index.insert(entry.url.clone(), entry);
        }

        Ok(())
    }

    /// Generate cache key for a schema URL
    fn cache_key(&self, url: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }

    /// Get file path for a schema
    fn file_path(&self, url: &str) -> PathBuf {
        let cache_key = self.cache_key(url);
        let filename = if self.config.binary_serialization {
            format!("{cache_key}.bin")
        } else {
            format!("{cache_key}.json")
        };

        self.cache_dir.join("schemas").join(filename)
    }

    /// Serialize schema for storage
    async fn serialize_schema(&self, schema: &FhirSchema) -> Result<Vec<u8>> {
        if self.config.binary_serialization {
            #[cfg(feature = "embedded-providers")]
            {
                // Use JSON for now due to serde_json::Value compatibility issues with bincode
                serde_json::to_vec(schema)
                    .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))
            }
            #[cfg(not(feature = "embedded-providers"))]
            {
                // Fallback to JSON if bincode is not available
                let json = serde_json::to_vec(schema)
                    .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))?;
                Ok(json)
            }
        } else {
            serde_json::to_vec_pretty(schema)
                .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))
        }
    }

    /// Deserialize schema from storage
    async fn deserialize_schema(&self, data: &[u8]) -> Result<FhirSchema> {
        if self.config.binary_serialization {
            #[cfg(feature = "embedded-providers")]
            {
                // Use JSON for now due to serde_json::Value compatibility issues with bincode
                serde_json::from_slice(data)
                    .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))
            }
            #[cfg(not(feature = "embedded-providers"))]
            {
                // Fallback to JSON
                serde_json::from_slice(data)
                    .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))
            }
        } else {
            serde_json::from_slice(data)
                .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))
        }
    }

    /// Compress data if compression is enabled
    async fn maybe_compress(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        if !self.config.enable_compression {
            return Ok(data);
        }

        #[cfg(feature = "compression")]
        {
            Ok(lz4_flex::compress_prepend_size(&data))
        }
        #[cfg(not(feature = "compression"))]
        {
            // No compression available, return as-is
            Ok(data)
        }
    }

    /// Decompress data if needed
    async fn maybe_decompress(&self, data: Vec<u8>) -> Result<Vec<u8>> {
        if !self.config.enable_compression {
            return Ok(data);
        }

        #[cfg(feature = "compression")]
        {
            lz4_flex::decompress_size_prepended(&data)
                .map_err(|e| FhirSchemaError::compression_error(&e.to_string()))
        }
        #[cfg(not(feature = "compression"))]
        {
            // No compression support, return as-is
            Ok(data)
        }
    }

    /// Store a cached package with fingerprinting
    pub async fn store_package(
        &mut self,
        package_id: &str,
        package_version: &str,
        schemas: Vec<FhirSchema>,
    ) -> Result<PackageFingerprint> {
        // Generate fingerprint for the package
        let serialized = serde_json::to_vec(&schemas)
            .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))?;
        let fingerprint = generate_package_fingerprint(package_id, package_version, &serialized);

        // Create package directory
        let package_dir = self
            .cache_dir
            .join("packages")
            .join(fingerprint.short_hash());

        fs::create_dir_all(&package_dir).await.map_err(|e| {
            FhirSchemaError::io_error(&format!("Failed to create package directory: {e}"))
        })?;

        // Store fingerprint
        let fingerprint_data = serde_json::to_vec_pretty(&fingerprint)
            .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))?;

        fs::write(package_dir.join("fingerprint.json"), fingerprint_data)
            .await
            .map_err(|e| FhirSchemaError::io_error(&format!("Failed to write fingerprint: {e}")))?;

        // Store each schema with its URL as key
        for schema in schemas {
            if let Some(url) = schema.id.clone() {
                self.store_schema(&url, schema).await?;
            }
        }

        Ok(fingerprint)
    }

    /// Load a cached package by fingerprint
    pub async fn load_package(
        &mut self,
        fingerprint: &PackageFingerprint,
    ) -> Result<Option<Vec<FhirSchema>>> {
        let package_dir = self
            .cache_dir
            .join("packages")
            .join(fingerprint.short_hash());

        // Check if package directory exists
        if !package_dir.exists() {
            return Ok(None);
        }

        // Load and verify fingerprint
        let fingerprint_path = package_dir.join("fingerprint.json");
        let fingerprint_data = fs::read_to_string(&fingerprint_path)
            .await
            .map_err(|e| FhirSchemaError::io_error(&format!("Failed to read fingerprint: {e}")))?;

        let cached_fingerprint: PackageFingerprint = serde_json::from_str(&fingerprint_data)
            .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))?;

        // Verify fingerprint matches
        if !cached_fingerprint.matches(fingerprint) {
            // Cache is invalid, remove it
            fs::remove_dir_all(&package_dir).await.ok();
            return Ok(None);
        }

        // Load all schemas from cache that match this package
        let mut schemas = Vec::new();
        for entry in self.index.values() {
            if entry.fingerprint.matches(&cached_fingerprint) {
                if let Some(schema) = self.get_schema(&entry.url).await? {
                    schemas.push(schema);
                }
            }
        }

        Ok(Some(schemas))
    }

    /// Check if a package is cached and valid
    pub async fn is_package_cached(&self, fingerprint: &PackageFingerprint) -> bool {
        let package_dir = self
            .cache_dir
            .join("packages")
            .join(fingerprint.short_hash());

        if !package_dir.exists() {
            return false;
        }

        // Check fingerprint validity
        let fingerprint_path = package_dir.join("fingerprint.json");
        if let Ok(fingerprint_data) = fs::read_to_string(&fingerprint_path).await {
            if let Ok(cached_fingerprint) =
                serde_json::from_str::<PackageFingerprint>(&fingerprint_data)
            {
                return cached_fingerprint.matches(fingerprint);
            }
        }

        false
    }

    /// Clear all cached data
    pub async fn clear_cache(&mut self) -> Result<()> {
        // Remove all files
        fs::remove_dir_all(&self.cache_dir)
            .await
            .map_err(|e| FhirSchemaError::io_error(&format!("Failed to clear cache: {e}")))?;

        // Recreate cache directory
        fs::create_dir_all(&self.cache_dir).await.map_err(|e| {
            FhirSchemaError::io_error(&format!("Failed to recreate cache directory: {e}"))
        })?;

        // Clear in-memory index
        self.index.clear();

        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        let mut total_size = 0u64;
        let mut file_count = 0usize;

        for entry in self.index.values() {
            total_size += entry.file_size;
            file_count += 1;
        }

        Ok(CacheStats {
            total_size,
            file_count,
            index_size: self.index.len(),
            cache_dir: self.cache_dir.clone(),
        })
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub total_size: u64,
    pub file_count: usize,
    pub index_size: usize,
    pub cache_dir: PathBuf,
}

#[async_trait]
impl SchemaStorage for DiskStorage {
    async fn store_schema(&self, url: &str, schema: FhirSchema) -> Result<()> {
        let file_path = self.file_path(url);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                FhirSchemaError::io_error(&format!("Failed to create schema directory: {e}"))
            })?;
        }

        // Serialize schema
        let data = self.serialize_schema(&schema).await?;
        let compressed_data = self.maybe_compress(data).await?;

        // Write to file
        fs::write(&file_path, &compressed_data)
            .await
            .map_err(|e| FhirSchemaError::io_error(&format!("Failed to write schema: {e}")))?;

        // Update index (we need mutable access, so this is a limitation of the trait)
        // In practice, DiskStorage should be wrapped in Arc<RwLock<DiskStorage>> for concurrent access

        Ok(())
    }

    async fn get_schema(&self, url: &str) -> Result<Option<FhirSchema>> {
        // Check if we have it in the index
        if let Some(entry) = self.index.get(url) {
            // Update last accessed time (would need mutable access in practice)
            let file_path = &entry.file_path;
            let full_path = self.cache_dir.join(file_path);

            if full_path.exists() {
                // Read and decompress file
                let compressed_data = fs::read(&full_path).await.map_err(|e| {
                    FhirSchemaError::io_error(&format!("Failed to read schema: {e}"))
                })?;

                let data = self.maybe_decompress(compressed_data).await?;
                let schema = self.deserialize_schema(&data).await?;

                return Ok(Some(schema));
            }
        }

        // Try reading directly by URL (fallback)
        let file_path = self.file_path(url);
        if file_path.exists() {
            let compressed_data = fs::read(&file_path)
                .await
                .map_err(|e| FhirSchemaError::io_error(&format!("Failed to read schema: {e}")))?;

            let data = self.maybe_decompress(compressed_data).await?;
            let schema = self.deserialize_schema(&data).await?;

            return Ok(Some(schema));
        }

        Ok(None)
    }

    async fn list_schemas(&self) -> Result<Vec<String>> {
        Ok(self.index.keys().cloned().collect())
    }

    async fn delete_schema(&self, url: &str) -> Result<()> {
        // Remove file if it exists
        let file_path = self.file_path(url);
        if file_path.exists() {
            fs::remove_file(&file_path)
                .await
                .map_err(|e| FhirSchemaError::io_error(&format!("Failed to delete schema: {e}")))?;
        }

        // Remove from index (would need mutable access in practice)

        Ok(())
    }
}
