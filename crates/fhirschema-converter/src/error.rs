//! Error types for FHIRSchema converter operations.

use thiserror::Error;

/// Result type alias for FHIRSchema converter operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error types that can occur in FHIRSchema converter operations.
#[derive(Error, Debug)]
pub enum Error {
    /// Core FHIRSchema error
    #[error("FHIRSchema core error: {0}")]
    Core(#[from] fhirschema_core::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// YAML serialization error
    #[error("YAML serialization error: {0}")]
    YamlSerialization(#[from] serde_yaml::Error),

    /// HTTP request error
    #[cfg(feature = "remote")]
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    /// Invalid StructureDefinition
    #[error("Invalid StructureDefinition: {0}")]
    InvalidStructureDefinition(String),

    /// Conversion error
    #[error("Conversion error: {0}")]
    Conversion(String),

    /// Reference resolution error
    #[error("Reference resolution error: {0}")]
    ReferenceResolution(String),
}
