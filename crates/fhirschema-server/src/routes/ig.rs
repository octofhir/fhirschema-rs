//! IG processing endpoints

use axum::{routing::{get, post, put}, Router};
use std::sync::Arc;

use crate::services::AppState;

/// Create IG routes
pub fn create_routes() -> Router {
    Router::new()
        .route("/process", post(process_ig))
        .route("/status/:job_id", get(ig_status))
        .route("/registry", get(list_registry_igs))
        .route("/schedule", put(configure_schedule))
}

/// Trigger manual IG processing
async fn process_ig() -> &'static str {
    "TODO: Implement IG processing"
}

/// Get IG processing job status
async fn ig_status() -> &'static str {
    "TODO: Implement IG status"
}

/// List available IGs from registry
async fn list_registry_igs() -> &'static str {
    "TODO: Implement list registry IGs"
}

/// Configure nightly processing schedule
async fn configure_schedule() -> &'static str {
    "TODO: Implement schedule configuration"
}
