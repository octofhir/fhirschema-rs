//! Core repository trait and data structures for FHIRSchema management

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fhirschema_core::Schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use uuid::Uuid;

use crate::{RepositoryError, RepositoryResult, SchemaVersion};

/// Repository metadata for tracking repository statistics
#[derive(Debug, Clone)]
pub struct RepositoryMetadata {
    /// Repository creation time
    pub created_at: chrono::DateTime<Utc>,
    /// Last update time
    pub last_updated: chrono::DateTime<Utc>,
    /// Total schemas count
    pub total_schemas: u64,
    /// Total versions count
    pub total_versions: u64,
    /// Total storage size in bytes (estimated)
    pub total_size: u64,
}

/// Core trait for FHIRSchema repository implementations
#[async_trait]
pub trait SchemaRepository: Send + Sync {
    /// Store a schema in the repository
    async fn store_schema(
        &self,
        schema: &Schema,
        metadata: Option<SchemaMetadata>,
    ) -> RepositoryResult<String>;

    /// Retrieve a schema by its canonical URL
    async fn get_schema(&self, url: &str) -> RepositoryResult<Schema>;

    /// Retrieve a specific version of a schema
    async fn get_schema_version(
        &self,
        url: &str,
        version: &SchemaVersion,
    ) -> RepositoryResult<Schema>;

    /// Get the latest version of a schema
    async fn get_latest_schema(&self, url: &str) -> RepositoryResult<Schema>;

    /// List all available schemas matching the query
    async fn list_schemas(&self, query: &SchemaQuery) -> RepositoryResult<Vec<SchemaMetadata>>;

    /// Check if a schema exists in the repository
    async fn schema_exists(&self, url: &str) -> RepositoryResult<bool>;

    /// Remove a schema from the repository
    async fn remove_schema(&self, url: &str) -> RepositoryResult<()>;

    /// Remove a specific version of a schema
    async fn remove_schema_version(
        &self,
        url: &str,
        version: &SchemaVersion,
    ) -> RepositoryResult<()>;

    /// Get metadata for a schema
    async fn get_metadata(&self, url: &str) -> RepositoryResult<SchemaMetadata>;

    /// Update metadata for a schema
    async fn update_metadata(
        &self,
        url: &str,
        metadata: &SchemaMetadata,
    ) -> RepositoryResult<()>;

    /// Search schemas by text query
    async fn search_schemas(&self, query: &str) -> RepositoryResult<Vec<SchemaMetadata>>;

    /// Get repository statistics
    async fn get_statistics(&self) -> RepositoryResult<RepositoryStatistics>;

    /// Validate repository integrity
    async fn validate_integrity(&self) -> RepositoryResult<IntegrityReport>;

    /// Cleanup unused or expired schemas
    async fn cleanup(&self, options: &CleanupOptions) -> RepositoryResult<CleanupReport>;
}

/// Metadata associated with a stored schema
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaMetadata {
    /// Unique identifier for this schema entry
    pub id: Uuid,

    /// Canonical URL of the schema
    pub url: String,

    /// Schema name
    pub name: Option<String>,

    /// Schema title
    pub title: Option<String>,

    /// Schema description
    pub description: Option<String>,

    /// Schema version
    pub version: SchemaVersion,

    /// Schema status (draft, active, retired, etc.)
    pub status: SchemaStatus,

    /// Schema type (resource, datatype, extension, etc.)
    pub schema_type: SchemaType,

    /// Base schema URL (for derived schemas)
    pub base: Option<String>,

    /// Schema derivation type
    pub derivation: Option<DerivationType>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Custom properties
    pub properties: HashMap<String, String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last modification timestamp
    pub updated_at: DateTime<Utc>,

    /// Schema size in bytes
    pub size: u64,

    /// Checksum for integrity verification
    pub checksum: String,

    /// Dependencies (other schemas this schema references)
    pub dependencies: Vec<String>,

    /// Dependents (other schemas that reference this schema)
    pub dependents: Vec<String>,
}

/// Schema status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SchemaStatus {
    Draft,
    Active,
    Retired,
    Unknown,
}

/// Schema type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SchemaType {
    Resource,
    DataType,
    Extension,
    Profile,
    LogicalModel,
    Unknown,
}

/// Schema derivation type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DerivationType {
    Specialization,
    Constraint,
    Extension,
    Unknown,
}

/// Query parameters for schema listing and searching
#[derive(Debug, Clone, Default)]
pub struct SchemaQuery {
    /// Filter by schema type
    pub schema_type: Option<SchemaType>,

    /// Filter by status
    pub status: Option<SchemaStatus>,

    /// Filter by base schema
    pub base: Option<String>,

    /// Filter by tags
    pub tags: Vec<String>,

    /// Text search in name, title, or description
    pub text: Option<String>,

    /// Filter by version range
    pub version_range: Option<VersionRange>,

    /// Limit number of results
    pub limit: Option<usize>,

    /// Offset for pagination
    pub offset: Option<usize>,

    /// Sort order
    pub sort: Option<SortOrder>,
}

/// Version range for filtering
#[derive(Debug, Clone)]
pub struct VersionRange {
    pub min: Option<SchemaVersion>,
    pub max: Option<SchemaVersion>,
}

/// Sort order for query results
#[derive(Debug, Clone)]
pub enum SortOrder {
    NameAsc,
    NameDesc,
    VersionAsc,
    VersionDesc,
    CreatedAsc,
    CreatedDesc,
    UpdatedAsc,
    UpdatedDesc,
}

/// Repository statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryStatistics {
    /// Total number of schemas
    pub total_schemas: u64,

    /// Total number of versions across all schemas
    pub total_versions: u64,

    /// Total storage size in bytes
    pub total_size: u64,

    /// Number of schemas by type
    pub schemas_by_type: HashMap<SchemaType, u64>,

    /// Number of schemas by status
    pub schemas_by_status: HashMap<SchemaStatus, u64>,

    /// Repository creation time
    pub created_at: DateTime<Utc>,

    /// Last update time
    pub last_updated: DateTime<Utc>,
}

/// Repository integrity report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    /// Overall integrity status
    pub is_valid: bool,

    /// List of integrity issues found
    pub issues: Vec<IntegrityIssue>,

    /// Number of schemas checked
    pub schemas_checked: u64,

    /// Check duration
    pub check_duration: std::time::Duration,
}

/// Individual integrity issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityIssue {
    /// Issue severity
    pub severity: IssueSeverity,

    /// Schema URL affected
    pub schema_url: String,

    /// Issue description
    pub description: String,

    /// Suggested fix
    pub suggested_fix: Option<String>,
}

/// Issue severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

/// Cleanup options
#[derive(Debug, Clone, Default)]
pub struct CleanupOptions {
    /// Remove schemas older than this duration
    pub max_age: Option<std::time::Duration>,

    /// Remove unused schemas (no dependents)
    pub remove_unused: bool,

    /// Remove draft schemas older than specified duration
    pub draft_max_age: Option<std::time::Duration>,

    /// Dry run mode (don't actually delete)
    pub dry_run: bool,
}

/// Cleanup operation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupReport {
    /// Number of schemas removed
    pub schemas_removed: u64,

    /// Number of versions removed
    pub versions_removed: u64,

    /// Storage space freed in bytes
    pub space_freed: u64,

    /// List of removed schema URLs
    pub removed_schemas: Vec<String>,

    /// Cleanup duration
    pub duration: std::time::Duration,
}

impl SchemaMetadata {
    /// Create new metadata for a schema
    pub fn new(url: String, version: SchemaVersion) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            url,
            name: None,
            title: None,
            description: None,
            version,
            status: SchemaStatus::Draft,
            schema_type: SchemaType::Unknown,
            base: None,
            derivation: None,
            tags: Vec::new(),
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
            size: 0,
            checksum: String::new(),
            dependencies: Vec::new(),
            dependents: Vec::new(),
        }
    }

    /// Update the modification timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if this schema is derived from another
    pub fn is_derived(&self) -> bool {
        self.base.is_some()
    }

    /// Check if this schema has dependencies
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }

    /// Check if this schema has dependents
    pub fn has_dependents(&self) -> bool {
        !self.dependents.is_empty()
    }
}

impl Default for SchemaStatus {
    fn default() -> Self {
        Self::Draft
    }
}

impl Default for SchemaType {
    fn default() -> Self {
        Self::Unknown
    }
}
