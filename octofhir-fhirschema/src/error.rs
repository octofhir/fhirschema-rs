use thiserror::Error;

#[derive(Error, Debug)]
pub enum FhirSchemaError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Missing element at path: {0}")]
    MissingElement(String),

    #[error("Invalid element type: expected {expected}, got {got}")]
    InvalidElementType { expected: String, got: String },

    #[error("Invalid cardinality: min={min}, max={max}")]
    InvalidCardinality { min: i32, max: i32 },

    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },

    #[error("Invalid choice element: {element}")]
    InvalidChoiceElement { element: String },

    #[error("Constraint violation: {constraint} - {message}")]
    ConstraintViolation { constraint: String, message: String },

    #[error("Reference validation failed: {target}")]
    ReferenceValidationFailed { target: String },

    #[error("Pattern mismatch: expected {expected}, got {got}")]
    PatternMismatch { expected: String, got: String },

    #[error("Unknown element: {element} at path {path}")]
    UnknownElement { element: String, path: String },

    #[error("Invalid slice configuration: {message}")]
    InvalidSlice { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("HTTP request error: {0}")]
    HttpError(String),

    #[error("Invalid FHIR version: {version}")]
    InvalidFhirVersion { version: String },

    #[error("Schema compilation error: {message}")]
    CompilationError { message: String },

    #[error("Conversion error: {message}")]
    ConversionError { message: String },

    #[error("Multiple validation errors")]
    MultipleErrors { errors: Vec<FhirSchemaError> },
}

pub type Result<T> = std::result::Result<T, FhirSchemaError>;

impl FhirSchemaError {
    pub fn invalid_path<S: Into<String>>(path: S) -> Self {
        Self::InvalidPath(path.into())
    }

    pub fn missing_element<S: Into<String>>(path: S) -> Self {
        Self::MissingElement(path.into())
    }

    pub fn invalid_element_type<S: Into<String>>(expected: S, got: S) -> Self {
        Self::InvalidElementType {
            expected: expected.into(),
            got: got.into(),
        }
    }

    pub fn invalid_cardinality(min: i32, max: i32) -> Self {
        Self::InvalidCardinality { min, max }
    }

    pub fn missing_required_field<S: Into<String>>(field: S) -> Self {
        Self::MissingRequiredField {
            field: field.into(),
        }
    }

    pub fn invalid_choice_element<S: Into<String>>(element: S) -> Self {
        Self::InvalidChoiceElement {
            element: element.into(),
        }
    }

    pub fn constraint_violation<S: Into<String>>(constraint: S, message: S) -> Self {
        Self::ConstraintViolation {
            constraint: constraint.into(),
            message: message.into(),
        }
    }

    pub fn reference_validation_failed<S: Into<String>>(target: S) -> Self {
        Self::ReferenceValidationFailed {
            target: target.into(),
        }
    }

    pub fn pattern_mismatch<S: Into<String>>(expected: S, got: S) -> Self {
        Self::PatternMismatch {
            expected: expected.into(),
            got: got.into(),
        }
    }

    pub fn unknown_element<S: Into<String>>(element: S, path: S) -> Self {
        Self::UnknownElement {
            element: element.into(),
            path: path.into(),
        }
    }

    pub fn invalid_slice<S: Into<String>>(message: S) -> Self {
        Self::InvalidSlice {
            message: message.into(),
        }
    }

    pub fn http_error<S: Into<String>>(message: S) -> Self {
        Self::HttpError(message.into())
    }

    pub fn invalid_fhir_version<S: Into<String>>(version: S) -> Self {
        Self::InvalidFhirVersion {
            version: version.into(),
        }
    }

    pub fn compilation_error<S: Into<String>>(message: S) -> Self {
        Self::CompilationError {
            message: message.into(),
        }
    }

    pub fn multiple_errors(errors: Vec<FhirSchemaError>) -> Self {
        Self::MultipleErrors { errors }
    }

    pub fn conversion_error<S: Into<String>>(message: S) -> Self {
        Self::ConversionError {
            message: message.into(),
        }
    }
}
