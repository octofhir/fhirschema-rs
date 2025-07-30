//! Error types and result handling for FHIRSchema validation

use thiserror::Error;

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Comprehensive error types for FHIRSchema validation
#[derive(Error, Debug)]
pub enum ValidationError {
    /// Schema resolution errors
    #[error("Schema resolution error: {message}")]
    SchemaResolution { message: String },

    /// Circular reference detected during schema resolution
    #[error("Circular reference detected in schema chain: {chain}")]
    CircularReference { chain: String },

    /// Schema not found
    #[error("Schema not found: {url}")]
    SchemaNotFound { url: String },

    /// Invalid schema structure
    #[error("Invalid schema structure: {message}")]
    InvalidSchema { message: String },

    /// Element validation errors
    #[error("Element validation error at {path}: {message}")]
    ElementValidation { path: String, message: String },

    /// Cardinality constraint violation
    #[error("Cardinality violation at {path}: expected {expected}, found {actual}")]
    CardinalityViolation {
        path: String,
        expected: String,
        actual: usize,
    },

    /// Type mismatch error
    #[error("Type mismatch at {path}: expected {expected}, found {actual}")]
    TypeMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    /// FHIRPath constraint evaluation errors
    #[error("FHIRPath constraint evaluation error: {message}")]
    FHIRPathError { message: String },

    /// FHIRPath expression parsing error
    #[error("FHIRPath expression parsing error: {expression} - {message}")]
    FHIRPathParseError { expression: String, message: String },

    /// Constraint violation
    #[error("Constraint violation at {path}: {constraint_key} - {message}")]
    ConstraintViolation {
        path: String,
        constraint_key: String,
        message: String,
    },

    /// Primitive datatype validation errors
    #[error("Primitive validation error at {path}: {message}")]
    PrimitiveValidation { path: String, message: String },

    /// Invalid format for primitive type
    #[error("Invalid format for {primitive_type} at {path}: {value}")]
    InvalidPrimitiveFormat {
        path: String,
        primitive_type: String,
        value: String,
    },

    /// UCUM unit validation error
    #[error("UCUM unit validation error at {path}: {message}")]
    UCUMError { path: String, message: String },

    /// Slicing validation errors
    #[error("Slicing validation error at {path}: {message}")]
    SlicingValidation { path: String, message: String },

    /// Slice matching error
    #[error("Slice matching error at {path}: no matching slice found for discriminator")]
    SliceMatchingError { path: String },

    /// Slice ordering violation
    #[error("Slice ordering violation at {path}: {message}")]
    SliceOrderingError { path: String, message: String },

    /// Resource structure errors
    #[error("Invalid resource structure: {message}")]
    InvalidResource { message: String },

    /// JSON/YAML parsing errors
    #[error("Parsing error: {message}")]
    ParseError { message: String },

    /// IO errors
    #[error("IO error: {message}")]
    IoError { message: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    /// Internal validation engine errors
    #[error("Internal validation error: {message}")]
    InternalError { message: String },
}

impl ValidationError {
    /// Create a schema resolution error
    pub fn schema_resolution(message: impl Into<String>) -> Self {
        Self::SchemaResolution {
            message: message.into(),
        }
    }

    /// Create a circular reference error
    pub fn circular_reference(chain: impl Into<String>) -> Self {
        Self::CircularReference {
            chain: chain.into(),
        }
    }

    /// Create a schema not found error
    pub fn schema_not_found(url: impl Into<String>) -> Self {
        Self::SchemaNotFound { url: url.into() }
    }

    /// Create an invalid schema error
    pub fn invalid_schema(message: impl Into<String>) -> Self {
        Self::InvalidSchema {
            message: message.into(),
        }
    }

    /// Create an element validation error
    pub fn element_validation(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ElementValidation {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a cardinality violation error
    pub fn cardinality_violation(
        path: impl Into<String>,
        expected: impl Into<String>,
        actual: usize,
    ) -> Self {
        Self::CardinalityViolation {
            path: path.into(),
            expected: expected.into(),
            actual,
        }
    }

    /// Create a type mismatch error
    pub fn type_mismatch(
        path: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::TypeMismatch {
            path: path.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create a FHIRPath error
    pub fn fhirpath_error(message: impl Into<String>) -> Self {
        Self::FHIRPathError {
            message: message.into(),
        }
    }

    /// Create a FHIRPath parse error
    pub fn fhirpath_parse_error(
        expression: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::FHIRPathParseError {
            expression: expression.into(),
            message: message.into(),
        }
    }

    /// Create a constraint violation error
    pub fn constraint_violation(
        path: impl Into<String>,
        constraint_key: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::ConstraintViolation {
            path: path.into(),
            constraint_key: constraint_key.into(),
            message: message.into(),
        }
    }

    /// Create a primitive validation error
    pub fn primitive_validation(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::PrimitiveValidation {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create an invalid primitive format error
    pub fn invalid_primitive_format(
        path: impl Into<String>,
        primitive_type: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self::InvalidPrimitiveFormat {
            path: path.into(),
            primitive_type: primitive_type.into(),
            value: value.into(),
        }
    }

    /// Create a UCUM error
    pub fn ucum_error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::UCUMError {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a slicing validation error
    pub fn slicing_validation(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SlicingValidation {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a slice matching error
    pub fn slice_matching_error(path: impl Into<String>) -> Self {
        Self::SliceMatchingError {
            path: path.into(),
        }
    }

    /// Create a slice ordering error
    pub fn slice_ordering_error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SliceOrderingError {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create an invalid resource error
    pub fn invalid_resource(message: impl Into<String>) -> Self {
        Self::InvalidResource {
            message: message.into(),
        }
    }

    /// Create a parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create an IO error
    pub fn io_error(message: impl Into<String>) -> Self {
        Self::IoError {
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }
}

// Convert from common error types
impl From<serde_json::Error> for ValidationError {
    fn from(err: serde_json::Error) -> Self {
        Self::parse_error(format!("JSON parsing error: {}", err))
    }
}

impl From<serde_yaml::Error> for ValidationError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::parse_error(format!("YAML parsing error: {}", err))
    }
}

impl From<std::io::Error> for ValidationError {
    fn from(err: std::io::Error) -> Self {
        Self::io_error(format!("IO error: {}", err))
    }
}

impl From<url::ParseError> for ValidationError {
    fn from(err: url::ParseError) -> Self {
        Self::parse_error(format!("URL parsing error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = ValidationError::schema_not_found("http://example.com/schema");
        assert!(matches!(err, ValidationError::SchemaNotFound { .. }));
    }

    #[test]
    fn test_error_display() {
        let err = ValidationError::type_mismatch("Patient.name", "string", "integer");
        let message = format!("{}", err);
        assert!(message.contains("Type mismatch"));
        assert!(message.contains("Patient.name"));
    }

    #[test]
    fn test_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json");
        assert!(json_err.is_err());

        let validation_err: ValidationError = json_err.unwrap_err().into();
        assert!(matches!(validation_err, ValidationError::ParseError { .. }));
    }
}
