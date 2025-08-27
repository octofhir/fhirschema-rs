//! # OctoFHIR FHIRSchema
//!
//! A high-performance Rust library for working with FHIRSchema that provides
//! conversion, validation, and package management capabilities.
//!
//! ## Features
//!
//! - **High Performance**: Async/await with parallel processing and adaptive batch sizing
//! - **Full Conversion**: Convert FHIR StructureDefinitions to FHIRSchema format
//! - **Validation**: Validate FHIR resources against FHIRSchema definitions
//! - **Package Management**: Advanced package management with registry and indexing
//! - **Storage**: Hierarchical caching with compression for optimal performance
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use octofhir_fhirschema::*;
//!
//! # async fn example() -> Result<()> {
//! // Convert a StructureDefinition to FHIRSchema
//! let structure_def: StructureDefinition = serde_json::from_str("{}")?;
//! let converter = FhirSchemaConverter::new();
//! let schema = converter.convert(&structure_def)?;
//!
//! // Validate a schema
//! let validator = BasicSchemaValidator;
//! let issues = validator.validate_schema(&schema)?;
//! # Ok(())
//! # }
//! ```

pub mod converter;
pub mod error;
pub mod package;
pub mod storage;
pub mod types;
pub mod validation;

#[cfg(feature = "cli")]
pub mod cli;

pub use converter::*;
pub use error::Result; // Our Result type takes precedence
pub use error::{ConversionError, FhirSchemaError, LoadError, ValidationError};
pub use package::{
    ConversionPipeline, FhirSchemaPackageManager, InstallOptions, ModelProvider, PackageId,
    PackageInstallResult, PackageManagerConfig, PackageSpec, ProgressTracker,
};
pub use storage::*;
pub use types::*;
pub use validation::{
    BasicSchemaValidator, FhirSchemaFieldValidator, FhirSchemaValidationEngine, FieldInfo,
    FieldValidationContext, FieldValidationResult, SchemaValidator, ValidationContext,
    ValidationEngine, ValidationResult,
};

// Re-export only the CanonicalManager from the official OctoFHIR canonical manager
pub use octofhir_canonical_manager::CanonicalManager;
