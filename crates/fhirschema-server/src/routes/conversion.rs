//! Conversion endpoints

use axum::{routing::{get, post}, Router};
use std::sync::Arc;

use crate::services::AppState;

/// Create conversion routes
pub fn create_routes() -> Router {
    Router::new()
        .route("/structuredefinition", post(convert_structure_definition))
        .route("/batch", post(convert_batch))
        .route("/status/:job_id", get(conversion_status))
        .route("/formats", get(supported_formats))
}

/// Convert StructureDefinition
async fn convert_structure_definition() -> &'static str {
    "TODO: Implement StructureDefinition conversion"
}

/// Batch conversion
async fn convert_batch() -> &'static str {
    "TODO: Implement batch conversion"
}

/// Get conversion status
async fn conversion_status() -> &'static str {
    "TODO: Implement conversion status"
}

/// Get supported formats
async fn supported_formats() -> &'static str {
    "TODO: Implement supported formats"
}
