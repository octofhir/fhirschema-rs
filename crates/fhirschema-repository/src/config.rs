//! Repository configuration and settings
//!
//! This module provides configuration management for different repository types
//! including memory, filesystem, and S3-compatible storage systems.

use crate::{
    error::{RepositoryError, RepositoryResult},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Filesystem repository configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSystemConfig {
    /// Root path for schema storage
    pub root_path: PathBuf,
    /// Create directory if missing
    pub create_if_missing: bool,
    /// Enable atomic operations
    pub atomic_operations: bool,
}

impl Default for FileSystemConfig {
    fn default() -> Self {
        Self {
            root_path: PathBuf::from("./schemas"),
            create_if_missing: true,
            atomic_operations: true,
        }
    }
}

/// S3 repository configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    /// S3 region
    pub region: Option<String>,
    /// Custom endpoint URL
    pub endpoint_url: Option<String>,
    /// Access key ID
    pub access_key_id: Option<String>,
    /// Secret access key
    pub secret_access_key: Option<String>,
    /// Path prefix
    pub prefix: String,
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
        }
    }
}

/// Repository type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepositoryType {
    Memory,
    FileSystem,
    S3,
}

impl std::fmt::Display for RepositoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryType::Memory => write!(f, "memory"),
            RepositoryType::FileSystem => write!(f, "filesystem"),
            RepositoryType::S3 => write!(f, "s3"),
        }
    }
}

impl std::str::FromStr for RepositoryType {
    type Err = RepositoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "memory" => Ok(RepositoryType::Memory),
            "filesystem" | "fs" => Ok(RepositoryType::FileSystem),
            "s3" => Ok(RepositoryType::S3),
            _ => Err(RepositoryError::configuration(format!(
                "Unknown repository type: {}",
                s
            ))),
        }
    }
}

/// Cache configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,
    /// Maximum number of cached items
    pub max_size: usize,
    /// Cache TTL in seconds
    pub ttl_seconds: u64,
    /// Enable persistent cache
    pub persistent: bool,
    /// Cache directory (for persistent cache)
    pub cache_dir: Option<PathBuf>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size: 1000,
            ttl_seconds: 3600, // 1 hour
            persistent: false,
            cache_dir: None,
        }
    }
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Connection timeout in seconds
    pub connection_timeout_seconds: u64,
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
    /// Maximum concurrent operations
    pub max_concurrent_operations: usize,
    /// Enable compression
    pub compression_enabled: bool,
    /// Batch size for bulk operations
    pub batch_size: usize,
    /// Enable parallel processing
    pub parallel_processing: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            connection_timeout_seconds: 30,
            request_timeout_seconds: 60,
            max_concurrent_operations: 10,
            compression_enabled: true,
            batch_size: 100,
            parallel_processing: true,
        }
    }
}

/// Security and access control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable authentication
    pub authentication_enabled: bool,
    /// Authentication method
    pub auth_method: Option<String>,
    /// Enable authorization
    pub authorization_enabled: bool,
    /// Enable TLS/SSL
    pub tls_enabled: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<PathBuf>,
    /// TLS key path
    pub tls_key_path: Option<PathBuf>,
    /// Enable request signing
    pub request_signing: bool,
    /// Access control rules
    pub access_rules: HashMap<String, Vec<String>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            authentication_enabled: false,
            auth_method: None,
            authorization_enabled: false,
            tls_enabled: true,
            tls_cert_path: None,
            tls_key_path: None,
            request_signing: false,
            access_rules: HashMap::new(),
        }
    }
}

/// Repository-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RepositorySpecificConfig {
    Memory {
        /// Maximum memory usage in MB
        max_memory_mb: Option<usize>,
    },
    FileSystem {
        /// Filesystem configuration
        #[serde(flatten)]
        config: FileSystemConfig,
    },
    S3 {
        /// S3 configuration
        #[serde(flatten)]
        config: S3Config,
    },
}

impl Default for RepositorySpecificConfig {
    fn default() -> Self {
        RepositorySpecificConfig::Memory {
            max_memory_mb: Some(512),
        }
    }
}

/// Complete repository configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryConfig {
    /// Repository name/identifier
    pub name: String,
    /// Repository type
    #[serde(rename = "type")]
    pub repository_type: RepositoryType,
    /// Repository-specific configuration
    #[serde(flatten)]
    pub specific: RepositorySpecificConfig,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Performance configuration
    pub performance: PerformanceConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Default for RepositoryConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            repository_type: RepositoryType::Memory,
            specific: RepositorySpecificConfig::default(),
            cache: CacheConfig::default(),
            performance: PerformanceConfig::default(),
            security: SecurityConfig::default(),
            metadata: HashMap::new(),
        }
    }
}

impl RepositoryConfig {
    /// Create a new memory repository configuration
    pub fn memory(name: &str) -> Self {
        Self {
            name: name.to_string(),
            repository_type: RepositoryType::Memory,
            specific: RepositorySpecificConfig::Memory {
                max_memory_mb: Some(512),
            },
            ..Default::default()
        }
    }

    /// Create a new filesystem repository configuration
    pub fn filesystem(name: &str, root_path: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            repository_type: RepositoryType::FileSystem,
            specific: RepositorySpecificConfig::FileSystem {
                config: FileSystemConfig {
                    root_path,
                    ..Default::default()
                },
            },
            ..Default::default()
        }
    }

    /// Create a new S3 repository configuration
    pub fn s3(name: &str, bucket: &str) -> Self {
        Self {
            name: name.to_string(),
            repository_type: RepositoryType::S3,
            specific: RepositorySpecificConfig::S3 {
                config: S3Config {
                    bucket: bucket.to_string(),
                    ..Default::default()
                },
            },
            ..Default::default()
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> RepositoryResult<()> {
        // Validate repository name
        if self.name.is_empty() {
            return Err(RepositoryError::configuration(
                "Repository name cannot be empty".to_string(),
            ));
        }

        // Validate repository-specific configuration
        match &self.specific {
            RepositorySpecificConfig::Memory { max_memory_mb } => {
                if let Some(memory) = max_memory_mb {
                    if *memory == 0 {
                        return Err(RepositoryError::configuration(
                            "Memory limit must be greater than 0".to_string(),
                        ));
                    }
                }
            }
            RepositorySpecificConfig::FileSystem { config } => {
                if !config.root_path.exists() && !config.create_if_missing {
                    return Err(RepositoryError::configuration(format!(
                        "Filesystem root path does not exist: {}",
                        config.root_path.display()
                    )));
                }
            }
            RepositorySpecificConfig::S3 { config } => {
                if config.bucket.is_empty() {
                    return Err(RepositoryError::configuration(
                        "S3 bucket name cannot be empty".to_string(),
                    ));
                }
            }
        }

        // Validate cache configuration
        if self.cache.enabled {
            if self.cache.max_size == 0 {
                return Err(RepositoryError::configuration(
                    "Cache max size must be greater than 0".to_string(),
                ));
            }
            if self.cache.ttl_seconds == 0 {
                return Err(RepositoryError::configuration(
                    "Cache TTL must be greater than 0".to_string(),
                ));
            }
        }

        // Validate performance configuration
        if self.performance.connection_timeout_seconds == 0 {
            return Err(RepositoryError::configuration(
                "Connection timeout must be greater than 0".to_string(),
            ));
        }
        if self.performance.request_timeout_seconds == 0 {
            return Err(RepositoryError::configuration(
                "Request timeout must be greater than 0".to_string(),
            ));
        }
        if self.performance.max_concurrent_operations == 0 {
            return Err(RepositoryError::configuration(
                "Max concurrent operations must be greater than 0".to_string(),
            ));
        }
        if self.performance.batch_size == 0 {
            return Err(RepositoryError::configuration(
                "Batch size must be greater than 0".to_string(),
            ));
        }

        // Validate security configuration
        if self.security.tls_enabled {
            if let Some(cert_path) = &self.security.tls_cert_path {
                if !cert_path.exists() {
                    return Err(RepositoryError::configuration(format!(
                        "TLS certificate file does not exist: {}",
                        cert_path.display()
                    )));
                }
            }
            if let Some(key_path) = &self.security.tls_key_path {
                if !key_path.exists() {
                    return Err(RepositoryError::configuration(format!(
                        "TLS key file does not exist: {}",
                        key_path.display()
                    )));
                }
            }
        }

        Ok(())
    }

    /// Get connection timeout as Duration
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_secs(self.performance.connection_timeout_seconds)
    }

    /// Get request timeout as Duration
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.performance.request_timeout_seconds)
    }

    /// Get cache TTL as Duration
    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.cache.ttl_seconds)
    }

    /// Check if caching is enabled
    pub fn is_cache_enabled(&self) -> bool {
        self.cache.enabled
    }

    /// Check if compression is enabled
    pub fn is_compression_enabled(&self) -> bool {
        self.performance.compression_enabled
    }

    /// Check if parallel processing is enabled
    pub fn is_parallel_processing_enabled(&self) -> bool {
        self.performance.parallel_processing
    }

    /// Get the filesystem configuration if this is a filesystem repository
    pub fn filesystem_config(&self) -> Option<&FileSystemConfig> {
        match &self.specific {
            RepositorySpecificConfig::FileSystem { config } => Some(config),
            _ => None,
        }
    }

    /// Get the S3 configuration if this is an S3 repository
    pub fn s3_config(&self) -> Option<&S3Config> {
        match &self.specific {
            RepositorySpecificConfig::S3 { config } => Some(config),
            _ => None,
        }
    }

    /// Get the memory limit if this is a memory repository
    pub fn memory_limit_mb(&self) -> Option<usize> {
        match &self.specific {
            RepositorySpecificConfig::Memory { max_memory_mb } => *max_memory_mb,
            _ => None,
        }
    }
}

/// Configuration manager for multiple repositories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigManager {
    /// Default repository name
    pub default_repository: String,
    /// Repository configurations
    pub repositories: HashMap<String, RepositoryConfig>,
    /// Global settings
    pub global: GlobalConfig,
}

/// Global configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Enable debug logging
    pub debug_logging: bool,
    /// Log level
    pub log_level: String,
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Metrics endpoint
    pub metrics_endpoint: Option<String>,
    /// Enable health checks
    pub health_checks_enabled: bool,
    /// Health check interval in seconds
    pub health_check_interval_seconds: u64,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            debug_logging: false,
            log_level: "info".to_string(),
            metrics_enabled: false,
            metrics_endpoint: None,
            health_checks_enabled: true,
            health_check_interval_seconds: 60,
        }
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        let mut repositories = HashMap::new();
        repositories.insert("default".to_string(), RepositoryConfig::default());

        Self {
            default_repository: "default".to_string(),
            repositories,
            global: GlobalConfig::default(),
        }
    }
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a repository configuration
    pub fn add_repository(&mut self, config: RepositoryConfig) -> RepositoryResult<()> {
        config.validate()?;
        self.repositories.insert(config.name.clone(), config);
        Ok(())
    }

    /// Remove a repository configuration
    pub fn remove_repository(&mut self, name: &str) -> bool {
        self.repositories.remove(name).is_some()
    }

    /// Get a repository configuration
    pub fn get_repository(&self, name: &str) -> Option<&RepositoryConfig> {
        self.repositories.get(name)
    }

    /// Get the default repository configuration
    pub fn get_default_repository(&self) -> Option<&RepositoryConfig> {
        self.repositories.get(&self.default_repository)
    }

    /// Set the default repository
    pub fn set_default_repository(&mut self, name: &str) -> RepositoryResult<()> {
        if !self.repositories.contains_key(name) {
            return Err(RepositoryError::configuration(format!(
                "Repository '{}' does not exist",
                name
            )));
        }
        self.default_repository = name.to_string();
        Ok(())
    }

    /// List all repository names
    pub fn list_repositories(&self) -> Vec<&String> {
        self.repositories.keys().collect()
    }

    /// Validate all configurations
    pub fn validate(&self) -> RepositoryResult<()> {
        // Validate that default repository exists
        if !self.repositories.contains_key(&self.default_repository) {
            return Err(RepositoryError::configuration(format!(
                "Default repository '{}' does not exist",
                self.default_repository
            )));
        }

        // Validate all repository configurations
        for (name, config) in &self.repositories {
            config.validate().map_err(|e| {
                RepositoryError::configuration(format!(
                    "Invalid configuration for repository '{}': {}",
                    name, e
                ))
            })?;
        }

        Ok(())
    }

    /// Load configuration from YAML file
    pub fn load_from_file(path: &std::path::Path) -> RepositoryResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            RepositoryError::configuration(format!("Failed to read config file: {}", e))
        })?;

        let config: ConfigManager = serde_yaml::from_str(&content).map_err(|e| {
            RepositoryError::configuration(format!("Failed to parse config file: {}", e))
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to YAML file
    pub fn save_to_file(&self, path: &std::path::Path) -> RepositoryResult<()> {
        self.validate()?;

        let content = serde_yaml::to_string(self).map_err(|e| {
            RepositoryError::configuration(format!("Failed to serialize config: {}", e))
        })?;

        std::fs::write(path, content).map_err(|e| {
            RepositoryError::configuration(format!("Failed to write config file: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_repository_type_from_str() {
        assert_eq!("memory".parse::<RepositoryType>().unwrap(), RepositoryType::Memory);
        assert_eq!("filesystem".parse::<RepositoryType>().unwrap(), RepositoryType::FileSystem);
        assert_eq!("fs".parse::<RepositoryType>().unwrap(), RepositoryType::FileSystem);
        assert_eq!("s3".parse::<RepositoryType>().unwrap(), RepositoryType::S3);
        assert!("invalid".parse::<RepositoryType>().is_err());
    }

    #[test]
    fn test_repository_config_validation() {
        let mut config = RepositoryConfig::default();
        assert!(config.validate().is_ok());

        // Test empty name
        config.name = "".to_string();
        assert!(config.validate().is_err());

        // Test invalid memory config
        config.name = "test".to_string();
        config.specific = RepositorySpecificConfig::Memory { max_memory_mb: Some(0) };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_manager() {
        let mut manager = ConfigManager::new();

        let config = RepositoryConfig::memory("test");
        assert!(manager.add_repository(config).is_ok());

        assert!(manager.get_repository("test").is_some());
        assert!(manager.remove_repository("test"));
        assert!(manager.get_repository("test").is_none());
    }

    #[test]
    fn test_config_file_operations() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yaml");

        let manager = ConfigManager::new();
        assert!(manager.save_to_file(&config_path).is_ok());

        let loaded_manager = ConfigManager::load_from_file(&config_path).unwrap();
        assert_eq!(manager.default_repository, loaded_manager.default_repository);
    }
}
