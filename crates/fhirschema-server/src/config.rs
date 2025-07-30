//! Server configuration management

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{Result, ServerError};

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server settings
    pub server: ServerSettings,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Redis configuration
    pub redis: RedisConfig,

    /// S3-compatible storage configuration
    pub storage: StorageConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// IG registry configuration
    pub ig_registry: IgRegistryConfig,

    /// Job processing configuration
    pub jobs: JobConfig,

    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
}

/// Server settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// Server host
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Maximum request body size in bytes
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Enable CORS
    #[serde(default = "default_true")]
    pub cors_enabled: bool,

    /// CORS allowed origins
    #[serde(default)]
    pub cors_origins: Vec<String>,

    /// Enable compression
    #[serde(default = "default_true")]
    pub compression_enabled: bool,

    /// Worker threads
    #[serde(default)]
    pub worker_threads: Option<usize>,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,

    /// Maximum connections
    #[serde(default = "default_db_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_db_timeout")]
    pub connect_timeout: u64,

    /// Enable migrations
    #[serde(default = "default_true")]
    pub run_migrations: bool,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    pub url: String,

    /// Maximum connections
    #[serde(default = "default_redis_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_redis_timeout")]
    pub connect_timeout: u64,

    /// Default TTL for cached items in seconds
    #[serde(default = "default_cache_ttl")]
    pub default_ttl: u64,
}

/// S3-compatible storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// S3 endpoint URL
    pub endpoint: String,

    /// S3 region
    #[serde(default = "default_region")]
    pub region: String,

    /// Access key ID
    pub access_key_id: String,

    /// Secret access key
    pub secret_access_key: String,

    /// Default bucket name
    pub bucket: String,

    /// Path prefix for schemas
    #[serde(default = "default_schema_prefix")]
    pub schema_prefix: String,

    /// Enable SSL/TLS
    #[serde(default = "default_true")]
    pub use_ssl: bool,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication
    #[serde(default = "default_false")]
    pub enabled: bool,

    /// JWT secret key
    pub jwt_secret: Option<String>,

    /// JWT expiration time in seconds
    #[serde(default = "default_jwt_expiration")]
    pub jwt_expiration: u64,

    /// API keys
    #[serde(default)]
    pub api_keys: Vec<String>,

    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Requests per minute
    #[serde(default = "default_rate_limit")]
    pub requests_per_minute: u32,

    /// Burst size
    #[serde(default = "default_burst_size")]
    pub burst_size: u32,
}

/// IG registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgRegistryConfig {
    /// Enable IG processing
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Registry endpoints
    #[serde(default = "default_ig_registries")]
    pub registries: Vec<String>,

    /// Processing schedule (cron expression)
    #[serde(default = "default_ig_schedule")]
    pub schedule: String,

    /// Maximum concurrent downloads
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_downloads: usize,

    /// Download timeout in seconds
    #[serde(default = "default_download_timeout")]
    pub download_timeout: u64,

    /// Retry attempts
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
}

/// Job processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    /// Maximum concurrent jobs
    #[serde(default = "default_max_jobs")]
    pub max_concurrent_jobs: usize,

    /// Job timeout in seconds
    #[serde(default = "default_job_timeout")]
    pub job_timeout: u64,

    /// Job retention time in seconds
    #[serde(default = "default_job_retention")]
    pub job_retention: u64,

    /// Enable job persistence
    #[serde(default = "default_true")]
    pub persist_jobs: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,

    /// Metrics endpoint path
    #[serde(default = "default_metrics_path")]
    pub metrics_path: String,

    /// Enable tracing
    #[serde(default = "default_true")]
    pub tracing_enabled: bool,

    /// Tracing endpoint
    pub tracing_endpoint: Option<String>,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Enable structured logging
    #[serde(default = "default_true")]
    pub structured_logging: bool,
}

/// Command line arguments
#[derive(Parser, Debug)]
#[command(name = "fhirschema-server")]
#[command(about = "FHIRSchema HTTP Server")]
pub struct Args {
    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Server host
    #[arg(long, env = "FHIRSCHEMA_HOST")]
    pub host: Option<String>,

    /// Server port
    #[arg(short, long, env = "FHIRSCHEMA_PORT")]
    pub port: Option<u16>,

    /// Database URL
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: Option<String>,

    /// Redis URL
    #[arg(long, env = "REDIS_URL")]
    pub redis_url: Option<String>,

    /// Log level
    #[arg(long, env = "RUST_LOG")]
    pub log_level: Option<String>,
}

impl ServerConfig {
    /// Load configuration from file and environment
    pub fn load(args: &Args) -> Result<Self> {
        let mut builder = config::Config::builder();

        // Load default configuration
        builder = builder.add_source(config::Config::try_from(&Self::default())?);

        // Load from configuration file if provided
        if let Some(config_path) = &args.config {
            builder = builder.add_source(config::File::from(config_path.clone()));
        }

        // Override with environment variables
        builder = builder.add_source(
            config::Environment::with_prefix("FHIRSCHEMA")
                .separator("_")
                .try_parsing(true)
        );

        let mut config: ServerConfig = builder.build()?.try_deserialize()?;

        // Override with command line arguments
        if let Some(host) = &args.host {
            config.server.host = host.clone();
        }
        if let Some(port) = args.port {
            config.server.port = port;
        }
        if let Some(database_url) = &args.database_url {
            config.database.url = database_url.clone();
        }
        if let Some(redis_url) = &args.redis_url {
            config.redis.url = redis_url.clone();
        }
        if let Some(log_level) = &args.log_level {
            config.monitoring.log_level = log_level.clone();
        }

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            return Err(ServerError::Config(config::ConfigError::Message(
                "Server port must be greater than 0".to_string()
            )));
        }

        if self.database.url.is_empty() {
            return Err(ServerError::Config(config::ConfigError::Message(
                "Database URL is required".to_string()
            )));
        }

        if self.redis.url.is_empty() {
            return Err(ServerError::Config(config::ConfigError::Message(
                "Redis URL is required".to_string()
            )));
        }

        if self.storage.endpoint.is_empty() {
            return Err(ServerError::Config(config::ConfigError::Message(
                "Storage endpoint is required".to_string()
            )));
        }

        Ok(())
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSettings::default(),
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            storage: StorageConfig::default(),
            auth: AuthConfig::default(),
            ig_registry: IgRegistryConfig::default(),
            jobs: JobConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            timeout: default_timeout(),
            max_body_size: default_max_body_size(),
            cors_enabled: default_true(),
            cors_origins: vec!["*".to_string()],
            compression_enabled: default_true(),
            worker_threads: None,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/fhirschema".to_string(),
            max_connections: default_db_max_connections(),
            connect_timeout: default_db_timeout(),
            run_migrations: default_true(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: default_redis_max_connections(),
            connect_timeout: default_redis_timeout(),
            default_ttl: default_cache_ttl(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:3900".to_string(),
            region: default_region(),
            access_key_id: "".to_string(),
            secret_access_key: "".to_string(),
            bucket: "fhirschema".to_string(),
            schema_prefix: default_schema_prefix(),
            use_ssl: default_true(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: default_false(),
            jwt_secret: None,
            jwt_expiration: default_jwt_expiration(),
            api_keys: vec![],
            rate_limit: RateLimitConfig::default(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            requests_per_minute: default_rate_limit(),
            burst_size: default_burst_size(),
        }
    }
}

impl Default for IgRegistryConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            registries: default_ig_registries(),
            schedule: default_ig_schedule(),
            max_concurrent_downloads: default_max_concurrent(),
            download_timeout: default_download_timeout(),
            retry_attempts: default_retry_attempts(),
        }
    }
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: default_max_jobs(),
            job_timeout: default_job_timeout(),
            job_retention: default_job_retention(),
            persist_jobs: default_true(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: default_true(),
            metrics_path: default_metrics_path(),
            tracing_enabled: default_true(),
            tracing_endpoint: None,
            log_level: default_log_level(),
            structured_logging: default_true(),
        }
    }
}

// Default value functions
fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_timeout() -> u64 { 30 }
fn default_max_body_size() -> usize { 16 * 1024 * 1024 } // 16MB
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_db_max_connections() -> u32 { 10 }
fn default_db_timeout() -> u64 { 30 }
fn default_redis_max_connections() -> u32 { 10 }
fn default_redis_timeout() -> u64 { 5 }
fn default_cache_ttl() -> u64 { 3600 } // 1 hour
fn default_region() -> String { "us-east-1".to_string() }
fn default_schema_prefix() -> String { "schemas/".to_string() }
fn default_jwt_expiration() -> u64 { 86400 } // 24 hours
fn default_rate_limit() -> u32 { 100 }
fn default_burst_size() -> u32 { 10 }
fn default_ig_registries() -> Vec<String> {
    vec!["https://packages.fhir.org".to_string()]
}
fn default_ig_schedule() -> String { "0 2 * * *".to_string() } // 2 AM daily
fn default_max_concurrent() -> usize { 5 }
fn default_download_timeout() -> u64 { 300 } // 5 minutes
fn default_retry_attempts() -> u32 { 3 }
fn default_max_jobs() -> usize { 10 }
fn default_job_timeout() -> u64 { 3600 } // 1 hour
fn default_job_retention() -> u64 { 86400 * 7 } // 7 days
fn default_metrics_path() -> String { "/metrics".to_string() }
fn default_log_level() -> String { "info".to_string() }
