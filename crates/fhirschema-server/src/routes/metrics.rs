//! Metrics endpoints

use axum::{response::Response, http::StatusCode};

/// Prometheus metrics handler
pub async fn metrics_handler() -> Result<Response<String>, StatusCode> {
    // TODO: Implement Prometheus metrics export
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
        .body("# TODO: Implement metrics\n".to_string())
        .unwrap())
}
