//! FHIRSchema HTTP Server
//!
//! This crate provides an HTTP server for FHIRSchema services, including:
//! - REST API for validation, conversion, and repository management
//! - Automated IG (Implementation Guide) processing with S3-compatible storage
//! - Async processing and job management
//! - Monitoring and observability

pub mod config;
pub mod server;
pub mod routes;
pub mod middleware;
pub mod services;
pub mod storage;
pub mod jobs;
pub mod auth;
pub mod metrics;
pub mod error;

pub use config::ServerConfig;
pub use server::Server;
pub use error::{ServerError, Result};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::{
        config::ServerConfig,
        server::Server,
        error::{ServerError, Result},
        services::*,
    };
}
