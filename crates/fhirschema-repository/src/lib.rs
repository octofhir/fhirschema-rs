//! # FHIRSchema Repository
//!
//! This crate provides repository and management capabilities for FHIRSchema definitions,
//! including storage, retrieval, versioning, and reference resolution.

pub mod error;
pub mod repository;
pub mod resolver;
pub mod version;
pub mod config;

#[cfg(feature = "memory")]
pub mod memory;

#[cfg(feature = "filesystem")]
pub mod filesystem;

#[cfg(feature = "s3")]
pub mod s3;

pub use error::{RepositoryError, RepositoryResult};
pub use repository::{SchemaRepository, SchemaMetadata, SchemaQuery, RepositoryMetadata};
pub use resolver::SchemaResolver;
pub use version::{SchemaVersion, VersionManager};
pub use config::{
    RepositoryConfig, RepositoryType, ConfigManager, CacheConfig,
    PerformanceConfig, SecurityConfig, GlobalConfig
};

#[cfg(feature = "memory")]
pub use memory::MemoryRepository;

#[cfg(feature = "filesystem")]
pub use filesystem::FileSystemRepository;

#[cfg(feature = "s3")]
pub use s3::{S3Repository, S3Config};
