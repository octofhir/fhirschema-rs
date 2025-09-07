use thiserror::Error;

pub type Result<T> = std::result::Result<T, FhirSchemaError>;

#[derive(Error, Debug)]
pub enum FhirSchemaError {
    #[error("Conversion failed for {resource_type}: {reason}")]
    ConversionFailed {
        resource_type: String,
        reason: String,
    },

    #[error("Validation failed with {error_count} errors")]
    ValidationFailed {
        error_count: usize,
        errors: Vec<ValidationError>,
    },

    #[error("Schema not found: {url}")]
    SchemaNotFound { url: String },

    #[error("Package installation failed: {package_id}")]
    PackageInstallationFailed {
        package_id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Package loading failed for {package}@{version}: {message}")]
    PackageLoadError {
        package: String,
        version: String,
        message: String,
    },

    #[error("Type resolution failed for {type_name}")]
    TypeResolutionFailed { type_name: String, context: String },

    #[error("Path navigation error: {message}")]
    PathError { message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Canonical manager error: {0}")]
    CanonicalManager(String),

    #[error("Storage error: {message}")]
    Storage { message: String },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Async runtime error: {message}")]
    Runtime { message: String },
}

#[derive(Error, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ValidationError {
    #[error("Constraint violation at {path}: {message}")]
    ConstraintViolation { path: String, message: String },

    #[error("Type mismatch at {path}: expected {expected}, found {actual}")]
    TypeMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("Required field missing at {path}")]
    RequiredFieldMissing { path: String },

    #[error("FHIRPath evaluation failed at {path}: {expression}")]
    FhirPathEvaluationFailed { path: String, expression: String },

    #[error("Invalid cardinality at {path}: {message}")]
    CardinalityViolation { path: String, message: String },
}

impl From<Box<dyn std::error::Error + Send + Sync>> for FhirSchemaError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        FhirSchemaError::Runtime {
            message: err.to_string(),
        }
    }
}

impl FhirSchemaError {
    pub fn conversion_failed(resource_type: &str, reason: &str) -> Self {
        FhirSchemaError::ConversionFailed {
            resource_type: resource_type.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn schema_not_found(url: &str) -> Self {
        FhirSchemaError::SchemaNotFound {
            url: url.to_string(),
        }
    }

    pub fn type_resolution_failed(type_name: &str, context: &str) -> Self {
        FhirSchemaError::TypeResolutionFailed {
            type_name: type_name.to_string(),
            context: context.to_string(),
        }
    }

    pub fn storage_error(message: &str) -> Self {
        FhirSchemaError::Storage {
            message: message.to_string(),
        }
    }

    pub fn configuration_error(message: &str) -> Self {
        FhirSchemaError::Configuration {
            message: message.to_string(),
        }
    }

    pub fn path_error(message: &str) -> Self {
        FhirSchemaError::PathError {
            message: message.to_string(),
        }
    }

    pub fn type_resolution_error(message: &str) -> Self {
        FhirSchemaError::TypeResolutionFailed {
            type_name: "unknown".to_string(),
            context: message.to_string(),
        }
    }

    pub fn navigation_failed(message: &str) -> Self {
        FhirSchemaError::PathError {
            message: message.to_string(),
        }
    }

    pub fn type_not_found(type_name: &str) -> Self {
        FhirSchemaError::SchemaNotFound {
            url: format!("http://hl7.org/fhir/StructureDefinition/{type_name}"),
        }
    }

    pub fn io_error(message: &str) -> Self {
        FhirSchemaError::Runtime {
            message: format!("IO error: {message}"),
        }
    }

    pub fn serialization_error(message: &str) -> Self {
        FhirSchemaError::Runtime {
            message: format!("Serialization error: {message}"),
        }
    }

    #[cfg(feature = "compression")]
    pub fn compression_error(message: &str) -> Self {
        FhirSchemaError::Runtime {
            message: format!("Compression error: {message}"),
        }
    }
}
