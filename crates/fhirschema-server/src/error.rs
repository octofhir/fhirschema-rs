//! Error handling for FHIRSchema server

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Result type alias for server operations
pub type Result<T> = std::result::Result<T, ServerError>;

/// Server error types
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conversion error: {0}")]
    Conversion(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Job error: {0}")]
    Job(String),

    #[error("IG registry error: {0}")]
    IgRegistry(String),

    #[error("S3 storage error: {0}")]
    S3Storage(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl ServerError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            ServerError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Validation(_) => StatusCode::BAD_REQUEST,
            ServerError::Conversion(_) => StatusCode::BAD_REQUEST,
            ServerError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Authentication(_) => StatusCode::UNAUTHORIZED,
            ServerError::Authorization(_) => StatusCode::FORBIDDEN,
            ServerError::RateLimit => StatusCode::TOO_MANY_REQUESTS,
            ServerError::Job(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::IgRegistry(_) => StatusCode::BAD_GATEWAY,
            ServerError::S3Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Redis(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::HttpClient(_) => StatusCode::BAD_GATEWAY,
            ServerError::Json(_) => StatusCode::BAD_REQUEST,
            ServerError::Yaml(_) => StatusCode::BAD_REQUEST,
            ServerError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServerError::NotFound(_) => StatusCode::NOT_FOUND,
            ServerError::Conflict(_) => StatusCode::CONFLICT,
            ServerError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Get the error code for API responses
    pub fn error_code(&self) -> &'static str {
        match self {
            ServerError::Config(_) => "CONFIG_ERROR",
            ServerError::Validation(_) => "VALIDATION_ERROR",
            ServerError::Conversion(_) => "CONVERSION_ERROR",
            ServerError::Storage(_) => "STORAGE_ERROR",
            ServerError::Authentication(_) => "AUTHENTICATION_ERROR",
            ServerError::Authorization(_) => "AUTHORIZATION_ERROR",
            ServerError::RateLimit => "RATE_LIMIT_EXCEEDED",
            ServerError::Job(_) => "JOB_ERROR",
            ServerError::IgRegistry(_) => "IG_REGISTRY_ERROR",
            ServerError::S3Storage(_) => "S3_STORAGE_ERROR",
            ServerError::Database(_) => "DATABASE_ERROR",
            ServerError::Redis(_) => "REDIS_ERROR",
            ServerError::HttpClient(_) => "HTTP_CLIENT_ERROR",
            ServerError::Json(_) => "JSON_ERROR",
            ServerError::Yaml(_) => "YAML_ERROR",
            ServerError::Io(_) => "IO_ERROR",
            ServerError::Internal(_) => "INTERNAL_ERROR",
            ServerError::BadRequest(_) => "BAD_REQUEST",
            ServerError::NotFound(_) => "NOT_FOUND",
            ServerError::Conflict(_) => "CONFLICT",
            ServerError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_code = self.error_code();
        let message = self.to_string();

        tracing::error!(
            error = %self,
            status = %status,
            error_code = error_code,
            "Server error occurred"
        );

        let body = Json(json!({
            "error": {
                "code": error_code,
                "message": message,
                "status": status.as_u16()
            }
        }));

        (status, body).into_response()
    }
}

/// Convert from redis errors
impl From<redis::RedisError> for ServerError {
    fn from(err: redis::RedisError) -> Self {
        ServerError::Redis(err.to_string())
    }
}

/// Convert from AWS SDK errors
impl From<aws_sdk_s3::Error> for ServerError {
    fn from(err: aws_sdk_s3::Error) -> Self {
        ServerError::S3Storage(err.to_string())
    }
}
