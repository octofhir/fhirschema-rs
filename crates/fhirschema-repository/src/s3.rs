//! S3-compatible repository implementation for FHIRSchema storage
//!
//! This module provides an S3Repository implementation that can work with
//! S3-compatible storage systems including AWS S3, MinIO, and Garage.

use crate::{
    error::{RepositoryError, RepositoryResult},
    repository::{SchemaRepository, SchemaMetadata, SchemaQuery, RepositoryMetadata},
    version::{SchemaVersion, VersionManager},
};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, Config};
use fhirschema_core::FhirSchema;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use url::Url;

/// Configuration for S3Repository
#[derive(Debug, Clone)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    /// S3 region (optional for S3-compatible services)
    pub region: Option<String>,
    /// Custom endpoint URL for S3-compatible services (e.g., Garage, MinIO)
    pub endpoint_url: Option<String>,
    /// Access key ID
    pub access_key_id: Option<String>,
    /// Secret access key
    pub secret_access_key: Option<String>,
    /// Path prefix for schema objects
    pub prefix: String,
    /// Enable path-style addressing (required for some S3-compatible services)
    pub path_style: bool,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            bucket: "fhirschema".to_string(),
            region: Some("us-east-1".to_string()),
            endpoint_url: None,
            access_key_id: None,
            secret_access_key: None,
            prefix: "schemas/".to_string(),
            path_style: false,
        }
    }
}

/// S3-compatible repository implementation
pub struct S3Repository {
    client: Client,
    config: S3Config,
    version_manager: Arc<RwLock<VersionManager>>,
    metadata_cache: Arc<RwLock<HashMap<String, SchemaMetadata>>>,
}

impl S3Repository {
    /// Create a new S3Repository with the given configuration
    pub async fn new(config: S3Config) -> RepositoryResult<Self> {
        let aws_config = Self::build_aws_config(&config).await?;
        let client = Client::new(&aws_config);

        // Verify bucket access
        Self::verify_bucket_access(&client, &config.bucket).await?;

        Ok(Self {
            client,
            config,
            version_manager: Arc::new(RwLock::new(VersionManager::new())),
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Build AWS configuration from S3Config
    async fn build_aws_config(config: &S3Config) -> RepositoryResult<aws_config::SdkConfig> {
        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());

        // Set region if provided
        if let Some(region) = &config.region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }

        // Set custom endpoint if provided (for S3-compatible services)
        if let Some(endpoint) = &config.endpoint_url {
            config_builder = config_builder.endpoint_url(endpoint);
        }

        // Set credentials if provided
        if let (Some(access_key), Some(secret_key)) = (&config.access_key_id, &config.secret_access_key) {
            let credentials = aws_sdk_s3::config::Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "fhirschema-repository",
            );
            config_builder = config_builder.credentials_provider(credentials);
        }

        Ok(config_builder.load().await)
    }

    /// Verify that we can access the specified bucket
    async fn verify_bucket_access(client: &Client, bucket: &str) -> RepositoryResult<()> {
        match client.head_bucket().bucket(bucket).send().await {
            Ok(_) => {
                info!("Successfully verified access to S3 bucket: {}", bucket);
                Ok(())
            }
            Err(e) => {
                error!("Failed to access S3 bucket '{}': {}", bucket, e);
                Err(RepositoryError::StorageError(format!(
                    "Cannot access S3 bucket '{}': {}",
                    bucket, e
                )))
            }
        }
    }

    /// Generate S3 object key for a schema
    fn schema_key(&self, url: &str, version: Option<&SchemaVersion>) -> String {
        let base_key = format!("{}{}", self.config.prefix, self.sanitize_url(url));
        match version {
            Some(v) => format!("{}/{}.json", base_key, v),
            None => format!("{}/latest.json", base_key),
        }
    }

    /// Generate S3 object key for schema metadata
    fn metadata_key(&self, url: &str) -> String {
        format!("{}{}/_metadata.json", self.config.prefix, self.sanitize_url(url))
    }

    /// Sanitize URL for use as S3 object key
    fn sanitize_url(&self, url: &str) -> String {
        url.replace("://", "/")
            .replace(":", "_")
            .replace("?", "_")
            .replace("#", "_")
            .replace(" ", "_")
    }

    /// Load schema metadata from S3
    async fn load_metadata(&self, url: &str) -> RepositoryResult<Option<SchemaMetadata>> {
        let key = self.metadata_key(url);

        match self.client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(response) => {
                let body = response.body.collect().await
                    .map_err(|e| RepositoryError::StorageError(format!("Failed to read metadata: {}", e)))?;
                let metadata: SchemaMetadata = serde_json::from_slice(&body.into_bytes())
                    .map_err(|e| RepositoryError::SerializationError(format!("Failed to deserialize metadata: {}", e)))?;
                Ok(Some(metadata))
            }
            Err(e) => {
                if e.to_string().contains("NoSuchKey") {
                    Ok(None)
                } else {
                    Err(RepositoryError::StorageError(format!("Failed to load metadata: {}", e)))
                }
            }
        }
    }

    /// Save schema metadata to S3
    async fn save_metadata(&self, url: &str, metadata: &SchemaMetadata) -> RepositoryResult<()> {
        let key = self.metadata_key(url);
        let data = serde_json::to_vec_pretty(metadata)
            .map_err(|e| RepositoryError::SerializationError(format!("Failed to serialize metadata: {}", e)))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(data.into())
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| RepositoryError::StorageError(format!("Failed to save metadata: {}", e)))?;

        // Update cache
        let mut cache = self.metadata_cache.write().await;
        cache.insert(url.to_string(), metadata.clone());

        Ok(())
    }

    /// List all schema objects with the given prefix
    async fn list_schemas(&self) -> RepositoryResult<Vec<String>> {
        let mut schemas = Vec::new();
        let mut continuation_token = None;

        loop {
            let mut request = self.client
                .list_objects_v2()
                .bucket(&self.config.bucket)
                .prefix(&self.config.prefix);

            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }

            let response = request.send().await
                .map_err(|e| RepositoryError::StorageError(format!("Failed to list objects: {}", e)))?;

            if let Some(contents) = response.contents {
                for object in contents {
                    if let Some(key) = object.key {
                        if key.ends_with(".json") && !key.contains("_metadata") {
                            schemas.push(key);
                        }
                    }
                }
            }

            if response.is_truncated == Some(true) {
                continuation_token = response.next_continuation_token;
            } else {
                break;
            }
        }

        Ok(schemas)
    }
}

#[async_trait]
impl SchemaRepository for S3Repository {
    async fn store_schema(
        &self,
        url: &str,
        schema: &FhirSchema,
        version: Option<&SchemaVersion>,
    ) -> RepositoryResult<()> {
        debug!("Storing schema in S3: {} (version: {:?})", url, version);

        // Serialize schema
        let schema_data = serde_json::to_vec_pretty(schema)
            .map_err(|e| RepositoryError::SerializationError(format!("Failed to serialize schema: {}", e)))?;

        // Store schema
        let schema_key = self.schema_key(url, version);
        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&schema_key)
            .body(schema_data.into())
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| RepositoryError::StorageError(format!("Failed to store schema: {}", e)))?;

        // Update version manager
        if let Some(v) = version {
            let mut version_manager = self.version_manager.write().await;
            version_manager.add_version(url, v.clone())?;
        }

        // Update or create metadata
        let mut metadata = self.load_metadata(url).await?.unwrap_or_else(|| {
            SchemaMetadata {
                url: url.to_string(),
                name: schema.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                description: schema.description.clone(),
                versions: Vec::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                tags: HashMap::new(),
            }
        });

        if let Some(v) = version {
            if !metadata.versions.contains(v) {
                metadata.versions.push(v.clone());
                metadata.versions.sort();
            }
        }
        metadata.updated_at = chrono::Utc::now();

        self.save_metadata(url, &metadata).await?;

        info!("Successfully stored schema: {} (version: {:?})", url, version);
        Ok(())
    }

    async fn get_schema(
        &self,
        url: &str,
        version: Option<&SchemaVersion>,
    ) -> RepositoryResult<Option<FhirSchema>> {
        debug!("Retrieving schema from S3: {} (version: {:?})", url, version);

        let key = self.schema_key(url, version);

        match self.client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(response) => {
                let body = response.body.collect().await
                    .map_err(|e| RepositoryError::StorageError(format!("Failed to read schema: {}", e)))?;
                let schema: FhirSchema = serde_json::from_slice(&body.into_bytes())
                    .map_err(|e| RepositoryError::SerializationError(format!("Failed to deserialize schema: {}", e)))?;

                debug!("Successfully retrieved schema: {}", url);
                Ok(Some(schema))
            }
            Err(e) => {
                if e.to_string().contains("NoSuchKey") {
                    debug!("Schema not found: {}", url);
                    Ok(None)
                } else {
                    error!("Failed to retrieve schema '{}': {}", url, e);
                    Err(RepositoryError::StorageError(format!("Failed to retrieve schema: {}", e)))
                }
            }
        }
    }

    async fn remove_schema(&self, url: &str, version: Option<&SchemaVersion>) -> RepositoryResult<bool> {
        debug!("Removing schema from S3: {} (version: {:?})", url, version);

        let key = self.schema_key(url, version);

        match self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => {
                // Update version manager
                if let Some(v) = version {
                    let mut version_manager = self.version_manager.write().await;
                    version_manager.remove_version(url, v)?;
                }

                // Update metadata
                if let Some(mut metadata) = self.load_metadata(url).await? {
                    if let Some(v) = version {
                        metadata.versions.retain(|existing| existing != v);
                    } else {
                        metadata.versions.clear();
                    }
                    metadata.updated_at = chrono::Utc::now();

                    if metadata.versions.is_empty() {
                        // Remove metadata if no versions left
                        let metadata_key = self.metadata_key(url);
                        let _ = self.client
                            .delete_object()
                            .bucket(&self.config.bucket)
                            .key(&metadata_key)
                            .send()
                            .await;

                        // Remove from cache
                        let mut cache = self.metadata_cache.write().await;
                        cache.remove(url);
                    } else {
                        self.save_metadata(url, &metadata).await?;
                    }
                }

                info!("Successfully removed schema: {} (version: {:?})", url, version);
                Ok(true)
            }
            Err(e) => {
                if e.to_string().contains("NoSuchKey") {
                    debug!("Schema not found for removal: {}", url);
                    Ok(false)
                } else {
                    error!("Failed to remove schema '{}': {}", url, e);
                    Err(RepositoryError::StorageError(format!("Failed to remove schema: {}", e)))
                }
            }
        }
    }

    async fn list_schemas(&self, query: Option<&SchemaQuery>) -> RepositoryResult<Vec<SchemaMetadata>> {
        debug!("Listing schemas from S3 with query: {:?}", query);

        let schema_keys = self.list_schemas().await?;
        let mut results = Vec::new();

        // Extract unique URLs from schema keys
        let mut urls = std::collections::HashSet::new();
        for key in schema_keys {
            if let Some(url_part) = key.strip_prefix(&self.config.prefix) {
                if let Some(url_end) = url_part.rfind('/') {
                    let url = &url_part[..url_end];
                    let restored_url = url.replace("/", "://").replace("_", ":");
                    urls.insert(restored_url);
                }
            }
        }

        // Load metadata for each unique URL
        for url in urls {
            if let Some(metadata) = self.load_metadata(&url).await? {
                // Apply query filters if provided
                if let Some(q) = query {
                    let mut matches = true;

                    if let Some(name_pattern) = &q.name_pattern {
                        if !metadata.name.contains(name_pattern) {
                            matches = false;
                        }
                    }

                    if let Some(url_pattern) = &q.url_pattern {
                        if !metadata.url.contains(url_pattern) {
                            matches = false;
                        }
                    }

                    if let Some(version_req) = &q.version {
                        if !metadata.versions.iter().any(|v| v == version_req) {
                            matches = false;
                        }
                    }

                    if matches {
                        results.push(metadata);
                    }
                } else {
                    results.push(metadata);
                }
            }
        }

        // Apply limit if specified
        if let Some(query) = query {
            if let Some(limit) = query.limit {
                results.truncate(limit);
            }
        }

        debug!("Found {} schemas matching query", results.len());
        Ok(results)
    }

    async fn get_versions(&self, url: &str) -> RepositoryResult<Vec<SchemaVersion>> {
        debug!("Getting versions for schema: {}", url);

        if let Some(metadata) = self.load_metadata(url).await? {
            Ok(metadata.versions)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_latest_version(&self, url: &str) -> RepositoryResult<Option<SchemaVersion>> {
        debug!("Getting latest version for schema: {}", url);

        let versions = self.get_versions(url).await?;
        Ok(versions.into_iter().max())
    }

    async fn get_metadata(&self, url: &str) -> RepositoryResult<Option<SchemaMetadata>> {
        debug!("Getting metadata for schema: {}", url);

        // Check cache first
        {
            let cache = self.metadata_cache.read().await;
            if let Some(metadata) = cache.get(url) {
                return Ok(Some(metadata.clone()));
            }
        }

        // Load from S3
        self.load_metadata(url).await
    }

    async fn update_metadata(&self, url: &str, metadata: &SchemaMetadata) -> RepositoryResult<()> {
        debug!("Updating metadata for schema: {}", url);
        self.save_metadata(url, metadata).await
    }

    async fn get_repository_metadata(&self) -> RepositoryResult<RepositoryMetadata> {
        debug!("Getting S3 repository metadata");

        let schemas = self.list_schemas(None).await?;
        let total_schemas = schemas.len();
        let mut total_versions = 0;

        for schema in &schemas {
            total_versions += schema.versions.len();
        }

        Ok(RepositoryMetadata {
            repository_type: "S3".to_string(),
            location: format!("s3://{}/{}", self.config.bucket, self.config.prefix),
            total_schemas,
            total_versions,
            created_at: chrono::Utc::now(), // This would ideally be stored separately
            last_updated: chrono::Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fhirschema_core::{FhirSchema, ElementDefinition};
    use tokio_test;

    fn create_test_schema() -> FhirSchema {
        FhirSchema {
            name: Some("TestSchema".to_string()),
            description: Some("A test schema".to_string()),
            elements: vec![ElementDefinition {
                path: "TestSchema".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_s3_config_default() {
        let config = S3Config::default();
        assert_eq!(config.bucket, "fhirschema");
        assert_eq!(config.prefix, "schemas/");
        assert!(!config.path_style);
    }

    #[tokio::test]
    async fn test_schema_key_generation() {
        let config = S3Config::default();
        let repo = S3Repository {
            client: Client::new(&aws_config::defaults(BehaviorVersion::latest()).load().await),
            config,
            version_manager: Arc::new(RwLock::new(VersionManager::new())),
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        let url = "http://example.com/schema";
        let version = SchemaVersion::new(1, 0, 0);

        let key = repo.schema_key(url, Some(&version));
        assert_eq!(key, "schemas/http/example.com/schema/1.0.0.json");

        let latest_key = repo.schema_key(url, None);
        assert_eq!(latest_key, "schemas/http/example.com/schema/latest.json");
    }

    #[tokio::test]
    async fn test_url_sanitization() {
        let config = S3Config::default();
        let repo = S3Repository {
            client: Client::new(&aws_config::defaults(BehaviorVersion::latest()).load().await),
            config,
            version_manager: Arc::new(RwLock::new(VersionManager::new())),
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        let url = "https://example.com:8080/path?query=value#fragment";
        let sanitized = repo.sanitize_url(url);
        assert_eq!(sanitized, "https/example.com_8080/path_query=value_fragment");
    }
}
