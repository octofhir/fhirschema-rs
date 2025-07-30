//! Error types for FHIRSchema core operations.

use thiserror::Error;

/// Result type alias for FHIRSchema core operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error types that can occur in FHIRSchema core operations.
#[derive(Error, Debug)]
pub enum Error {
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// YAML serialization error
    #[error("YAML serialization error: {0}")]
    YamlSerialization(#[from] serde_yaml::Error),

    /// Invalid schema structure
    #[error("Invalid schema structure: {0}")]
    InvalidSchema(String),

    /// Invalid element definition
    #[error("Invalid element definition: {0}")]
    InvalidElement(String),

    /// Invalid constraint
    #[error("Invalid constraint: {0}")]
    InvalidConstraint(String),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),
}
