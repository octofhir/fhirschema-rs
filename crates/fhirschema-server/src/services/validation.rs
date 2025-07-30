//! Validation service

use moka::future::Cache;
use sqlx::PgPool;
use std::sync::Arc;

use crate::metrics::Metrics;

/// Validation service
pub struct ValidationService {
    db: PgPool,
    cache: Cache<String, Vec<u8>>,
    metrics: Arc<Metrics>,
}

impl ValidationService {
    /// Create new validation service
    pub fn new(db: PgPool, cache: Cache<String, Vec<u8>>, metrics: Arc<Metrics>) -> Self {
        Self { db, cache, metrics }
    }

    // TODO: Implement validation methods
}
