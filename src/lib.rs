pub mod converter;
pub mod error;
pub mod storage;
pub mod types;
pub mod validation;

pub use converter::*;
pub use error::Result; // Our Result type takes precedence
pub use error::{ConversionError, FhirSchemaError, LoadError, ValidationError};
pub use storage::*;
pub use types::*;
pub use validation::{
    BasicSchemaValidator, FhirSchemaValidationEngine, SchemaValidator, ValidationContext,
    ValidationEngine, ValidationResult,
};

// Re-export only the CanonicalManager from the official OctoFHIR canonical manager
pub use octofhir_canonical_manager::CanonicalManager;
