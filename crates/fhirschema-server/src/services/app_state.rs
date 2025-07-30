//! Application state management

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use moka::future::Cache;
use redis::aio::ConnectionManager;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

use crate::{
    config::ServerConfig,
    error::{Result, ServerError},
    jobs::JobManager,
    metrics::Metrics,
    storage::S3Storage,
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub db: PgPool,

    /// Redis connection manager
    pub redis: ConnectionManager,

    /// S3-compatible storage client
    pub storage: Arc<S3Storage>,

    /// In-memory cache
    pub cache: Cache<String, Vec<u8>>,

    /// Job manager
    pub job_manager: Arc<JobManager>,

    /// Metrics collector
    pub metrics: Arc<Metrics>,

    /// Server configuration
    pub config: ServerConfig,

    /// Validation service
    pub validation_service: Arc<super::validation::ValidationService>,

    /// Conversion service
    pub conversion_service: Arc<super::conversion::ConversionService>,

    /// IG registry service
    pub ig_registry_service: Arc<super::ig_registry::IgRegistryService>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: &ServerConfig) -> Result<Self> {
        // Initialize database connection pool
        let db = Self::init_database(config).await?;

        // Initialize Redis connection
        let redis = Self::init_redis(config).await?;

        // Initialize S3 storage
        let storage = Arc::new(Self::init_storage(config).await?);

        // Initialize in-memory cache
        let cache = Self::init_cache(config);

        // Initialize metrics
        let metrics = Arc::new(Metrics::new());

        // Initialize job manager
        let job_manager = Arc::new(JobManager::new(
            db.clone(),
            redis.clone(),
            &config.jobs,
        ).await?);

        // Initialize services
        let validation_service = Arc::new(
            super::validation::ValidationService::new(
                db.clone(),
                cache.clone(),
                metrics.clone(),
            )
        );

        let conversion_service = Arc::new(
            super::conversion::ConversionService::new(
                db.clone(),
                cache.clone(),
                metrics.clone(),
            )
        );

        let ig_registry_service = Arc::new(
            super::ig_registry::IgRegistryService::new(
                storage.clone(),
                job_manager.clone(),
                &config.ig_registry,
            ).await?
        );

        Ok(Self {
            db,
            redis,
            storage,
            cache,
            job_manager,
            metrics,
            config: config.clone(),
            validation_service,
            conversion_service,
            ig_registry_service,
        })
    }

    /// Initialize database connection pool
    async fn init_database(config: &ServerConfig) -> Result<PgPool> {
        let pool = PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .acquire_timeout(Duration::from_secs(config.database.connect_timeout))
            .connect(&config.database.url)
            .await
            .map_err(|e| ServerError::Database(e))?;

        // Run migrations if enabled
        if config.database.run_migrations {
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .map_err(|e| ServerError::Database(e))?;
        }

        Ok(pool)
    }

    /// Initialize Redis connection
    async fn init_redis(config: &ServerConfig) -> Result<ConnectionManager> {
        let client = redis::Client::open(config.redis.url.as_str())
            .map_err(|e| ServerError::Redis(e.to_string()))?;

        let manager = ConnectionManager::new(client)
            .await
            .map_err(|e| ServerError::Redis(e.to_string()))?;

        Ok(manager)
    }

    /// Initialize S3-compatible storage
    async fn init_storage(config: &ServerConfig) -> Result<S3Storage> {
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(&config.storage.endpoint)
            .region(aws_config::Region::new(config.storage.region.clone()))
            .credentials_provider(aws_config::meta::credentials::CredentialsProvider::new(
                aws_config::meta::credentials::StaticCredentialsProvider::new(
                    config.storage.access_key_id.clone(),
                    config.storage.secret_access_key.clone(),
                    None,
                )
            ))
            .load()
            .await;

        let s3_client = S3Client::new(&aws_config);

        Ok(S3Storage::new(
            s3_client,
            config.storage.bucket.clone(),
            config.storage.schema_prefix.clone(),
        ))
    }

    /// Initialize in-memory cache
    fn init_cache(config: &ServerConfig) -> Cache<String, Vec<u8>> {
        Cache::builder()
            .max_capacity(10_000) // Maximum 10k entries
            .time_to_live(Duration::from_secs(config.redis.default_ttl))
            .build()
    }

    /// Get database connection pool
    pub fn db(&self) -> &PgPool {
        &self.db
    }

    /// Get Redis connection manager
    pub fn redis(&self) -> &ConnectionManager {
        &self.redis
    }

    /// Get storage client
    pub fn storage(&self) -> &S3Storage {
        &self.storage
    }

    /// Get cache
    pub fn cache(&self) -> &Cache<String, Vec<u8>> {
        &self.cache
    }

    /// Get job manager
    pub fn job_manager(&self) -> &JobManager {
        &self.job_manager
    }

    /// Get metrics collector
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Get validation service
    pub fn validation_service(&self) -> &super::validation::ValidationService {
        &self.validation_service
    }

    /// Get conversion service
    pub fn conversion_service(&self) -> &super::conversion::ConversionService {
        &self.conversion_service
    }

    /// Get IG registry service
    pub fn ig_registry_service(&self) -> &super::ig_registry::IgRegistryService {
        &self.ig_registry_service
    }

    /// Shutdown application state gracefully
    pub async fn shutdown(&self) -> Result<()> {
        // Stop job manager
        self.job_manager.shutdown().await?;

        // Close database connections
        self.db.close().await;

        Ok(())
    }
}
