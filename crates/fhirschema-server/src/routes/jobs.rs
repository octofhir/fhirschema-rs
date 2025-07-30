//! Job management endpoints

use axum::{routing::{get, post, delete}, Router};
use std::sync::Arc;

use crate::services::AppState;

/// Create job routes
pub fn create_routes() -> Router {
    Router::new()
        .route("/", get(list_jobs))
        .route("/:job_id", get(get_job).delete(cancel_job))
        .route("/:job_id/status", get(job_status))
        .route("/:job_id/result", get(job_result))
}

/// List all jobs
async fn list_jobs() -> &'static str {
    "TODO: Implement list jobs"
}

/// Get specific job
async fn get_job() -> &'static str {
    "TODO: Implement get job"
}

/// Get job status
async fn job_status() -> &'static str {
    "TODO: Implement job status"
}

/// Get job result
async fn job_result() -> &'static str {
    "TODO: Implement job result"
}

/// Cancel job
async fn cancel_job() -> &'static str {
    "TODO: Implement cancel job"
}
