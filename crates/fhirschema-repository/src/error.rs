//! Error types for FHIRSchema repository operations

use std::fmt;
use thiserror::Error;

/// Result type for repository operations
pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Comprehensive error types for repository operations
#[derive(Error, Debug)]
pub enum RepositoryError {
    /// Schema not found in repository
    #[error("Schema not found: {url}")]
    SchemaNotFound { url: String },

    /// Schema version not found
    #[error("Schema version not found: {url} version {version}")]
    VersionNotFound { url: String, version: String },

    /// Invalid schema format or content
    #[error("Invalid schema: {message}")]
    InvalidSchema { message: String },

    /// Circular dependency detected
    #[error("Circular dependency detected in schema chain: {chain}")]
    CircularDependency { chain: String },

    /// Version conflict during operation
    #[error("Version conflict: {message}")]
    VersionConflict { message: String },

    /// Repository configuration error
    #[error("Repository configuration error: {message}")]
    Configuration { message: String },

    /// I/O error during repository operations
    #[error("I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// Serialization/deserialization error
    #[error("Serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    /// YAML serialization error
    #[error("YAML error: {source}")]
    Yaml {
        #[from]
        source: serde_yaml::Error,
    },

    /// URL parsing error
    #[error("URL parsing error: {source}")]
    UrlParse {
        #[from]
        source: url::ParseError,
    },

    /// Network/HTTP error for remote operations
    #[cfg(feature = "remote")]
    #[error("Network error: {source}")]
    Network {
        #[from]
        source: reqwest::Error,
    },

    /// AWS S3 error for S3 repository
    #[cfg(feature = "s3")]
    #[error("S3 error: {message}")]
    S3 { message: String },

    /// Concurrent access error
    #[error("Concurrent access error: {message}")]
    Concurrency { message: String },

    /// Repository is read-only
    #[error("Repository is read-only")]
    ReadOnly,

    /// Storage quota exceeded
    #[error("Storage quota exceeded: {current}/{limit} bytes")]
    QuotaExceeded { current: u64, limit: u64 },

    /// Generic repository error
    #[error("Repository error: {message}")]
    Generic { message: String },
}

impl RepositoryError {
    /// Create a new schema not found error
    pub fn schema_not_found(url: impl Into<String>) -> Self {
        Self::SchemaNotFound { url: url.into() }
    }

    /// Create a new version not found error
    pub fn version_not_found(url: impl Into<String>, version: impl Into<String>) -> Self {
        Self::VersionNotFound {
            url: url.into(),
            version: version.into(),
        }
    }

    /// Create a new invalid schema error
    pub fn invalid_schema(message: impl Into<String>) -> Self {
        Self::InvalidSchema {
            message: message.into(),
        }
    }

    /// Create a new circular dependency error
    pub fn circular_dependency(chain: impl Into<String>) -> Self {
        Self::CircularDependency {
            chain: chain.into(),
        }
    }

    /// Create a new version conflict error
    pub fn version_conflict(message: impl Into<String>) -> Self {
        Self::VersionConflict {
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a new concurrency error
    pub fn concurrency(message: impl Into<String>) -> Self {
        Self::Concurrency {
            message: message.into(),
        }
    }

    /// Create a new generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::SchemaNotFound { .. } => false,
            Self::VersionNotFound { .. } => false,
            Self::InvalidSchema { .. } => false,
            Self::CircularDependency { .. } => false,
            Self::ReadOnly => false,
            Self::Configuration { .. } => false,
            Self::Io { .. } => true,
            Self::Serialization { .. } => false,
            Self::Yaml { .. } => false,
            Self::UrlParse { .. } => false,
            #[cfg(feature = "remote")]
            Self::Network { .. } => true,
            #[cfg(feature = "s3")]
            Self::S3 { .. } => true,
            Self::Concurrency { .. } => true,
            Self::VersionConflict { .. } => true,
            Self::QuotaExceeded { .. } => false,
            Self::Generic { .. } => false,
        }
    }

    /// Get error category for logging/metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::SchemaNotFound { .. } => "not_found",
            Self::VersionNotFound { .. } => "not_found",
            Self::InvalidSchema { .. } => "validation",
            Self::CircularDependency { .. } => "validation",
            Self::VersionConflict { .. } => "conflict",
            Self::Configuration { .. } => "configuration",
            Self::Io { .. } => "io",
            Self::Serialization { .. } => "serialization",
            Self::Yaml { .. } => "serialization",
            Self::UrlParse { .. } => "validation",
            #[cfg(feature = "remote")]
            Self::Network { .. } => "network",
            #[cfg(feature = "s3")]
            Self::S3 { .. } => "storage",
            Self::Concurrency { .. } => "concurrency",
            Self::ReadOnly => "permission",
            Self::QuotaExceeded { .. } => "quota",
            Self::Generic { .. } => "generic",
        }
    }
}
