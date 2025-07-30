//! Schema repository endpoints

use axum::{routing::{get, post, put, delete}, Router};
use std::sync::Arc;

use crate::services::AppState;

/// Create schema routes
pub fn create_routes() -> Router {
    Router::new()
        .route("/", get(list_schemas).post(upload_schema))
        .route("/:id", get(get_schema).put(update_schema).delete(delete_schema))
}

/// List available schemas
async fn list_schemas() -> &'static str {
    "TODO: Implement list schemas"
}

/// Get specific schema
async fn get_schema() -> &'static str {
    "TODO: Implement get schema"
}

/// Upload new schema
async fn upload_schema() -> &'static str {
    "TODO: Implement upload schema"
}

/// Update existing schema
async fn update_schema() -> &'static str {
    "TODO: Implement update schema"
}

/// Delete schema
async fn delete_schema() -> &'static str {
    "TODO: Implement delete schema"
}
