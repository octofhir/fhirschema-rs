//! Conversion service

use moka::future::Cache;
use sqlx::PgPool;
use std::sync::Arc;

use crate::metrics::Metrics;

/// Conversion service
pub struct ConversionService {
    db: PgPool,
    cache: Cache<String, Vec<u8>>,
    metrics: Arc<Metrics>,
}

impl ConversionService {
    /// Create new conversion service
    pub fn new(db: PgPool, cache: Cache<String, Vec<u8>>, metrics: Arc<Metrics>) -> Self {
        Self { db, cache, metrics }
    }

    // TODO: Implement conversion methods
}
