//! Error types for FHIRSchema code generation

use thiserror::Error;

/// Result type for code generation operations
pub type CodegenResult<T> = Result<T, CodegenError>;

/// Errors that can occur during code generation
#[derive(Error, Debug)]
pub enum CodegenError {
    /// Schema parsing or validation error
    #[error("Schema error: {0}")]
    SchemaError(String),

    /// Template processing error
    #[error("Template error: {0}")]
    TemplateError(String),

    /// File I/O error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Code generation error
    #[error("Generation error: {0}")]
    GenerationError(String),

    /// Template rendering error
    #[error("Template rendering error: {0}")]
    RenderError(#[from] handlebars::RenderError),

    /// Template compilation error
    #[error("Template compilation error: {0}")]
    TemplateCompileError(#[from] handlebars::TemplateError),

    /// Unsupported feature error
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    /// Invalid input error
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl CodegenError {
    /// Create a new schema error
    pub fn schema_error(msg: impl Into<String>) -> Self {
        Self::SchemaError(msg.into())
    }

    /// Create a new template error
    pub fn template_error(msg: impl Into<String>) -> Self {
        Self::TemplateError(msg.into())
    }

    /// Create a new configuration error
    pub fn config_error(msg: impl Into<String>) -> Self {
        Self::ConfigError(msg.into())
    }

    /// Create a new generation error
    pub fn generation_error(msg: impl Into<String>) -> Self {
        Self::GenerationError(msg.into())
    }

    /// Create a new unsupported feature error
    pub fn unsupported_feature(msg: impl Into<String>) -> Self {
        Self::UnsupportedFeature(msg.into())
    }

    /// Create a new invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }
}
