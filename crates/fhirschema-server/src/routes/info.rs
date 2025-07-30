//! Server info endpoints

use axum::{response::Json, http::StatusCode};
use serde_json::{json, Value};

/// Server information endpoint
pub async fn server_info() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "name": "fhirschema-server",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "FHIRSchema HTTP Server",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}
