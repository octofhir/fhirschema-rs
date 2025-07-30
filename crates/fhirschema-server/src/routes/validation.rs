//! Validation endpoints

use axum::{routing::post, Router};
use std::sync::Arc;

use crate::services::AppState;

/// Create validation routes
pub fn create_routes() -> Router {
    Router::new()
        .route("/", post(validate_resource))
        .route("/batch", post(validate_batch))
        .route("/status/:job_id", post(validation_status))
}

/// Validate FHIR resource
async fn validate_resource() -> &'static str {
    "TODO: Implement validation"
}

/// Batch validation
async fn validate_batch() -> &'static str {
    "TODO: Implement batch validation"
}

/// Get validation status
async fn validation_status() -> &'static str {
    "TODO: Implement validation status"
}
