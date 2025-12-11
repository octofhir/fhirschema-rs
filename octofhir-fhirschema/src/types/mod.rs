//! Core type definitions for octofhir-fhirschema.
//!
//! This module contains all the type definitions used throughout the crate:
//!
//! - **[`schema`]** - FHIR Schema types ([`FhirSchema`], [`FhirSchemaElement`], etc.)
//! - **[`validation`]** - Validation result types ([`ValidationResult`], [`ValidationError`])
//! - **[`structure_definition`]** - StructureDefinition types for conversion
//!
//! # Core Types
//!
//! The main types you'll work with are:
//!
//! - [`FhirSchema`] - Represents a FHIR Schema (resource, profile, or type)
//! - [`FhirSchemaElement`] - Represents an element definition within a schema
//! - [`ValidationResult`] - Contains the result of validating a resource
//! - [`ValidationError`] - A single validation error or warning
//! - [`StructureDefinition`] - FHIR StructureDefinition for conversion to schema
//!
//! # Example
//!
//! ```ignore
//! use octofhir_fhirschema::types::{FhirSchema, ValidationResult};
//!
//! // Load a schema
//! let schema: FhirSchema = serde_json::from_str(json_str)?;
//!
//! // After validation
//! let result: ValidationResult = validator.validate(&resource, vec!["Patient".to_string()]).await;
//! if !result.valid {
//!     for error in &result.errors {
//!         println!("Error at {:?}: {}", error.path, error);
//!     }
//! }
//! ```

pub mod schema;
pub mod structure_definition;
pub mod validation;

// Re-export commonly used types at the module level
pub use schema::{
    FHIR_COMPLEX_TYPES, FHIR_PRIMITIVE_TYPES, FhirSchema, FhirSchemaBinding, FhirSchemaConstraint,
    FhirSchemaDiscriminator, FhirSchemaElement, FhirSchemaPattern, FhirSchemaSliceMatch,
    FhirSchemaSlicing, is_fhir_schema, is_fhir_schema_element,
};

pub use structure_definition::{
    Action, ConversionContext, PathComponent, StructureDefinition, StructureDefinitionBase,
    StructureDefinitionBinding, StructureDefinitionConstraint, StructureDefinitionDifferential,
    StructureDefinitionDiscriminator, StructureDefinitionElement, StructureDefinitionExtension,
    StructureDefinitionSlicing, StructureDefinitionSnapshot, StructureDefinitionType,
    is_structure_definition,
};

pub use validation::{
    VALIDATION_ERROR_TYPES, ValidationContext, ValidationError, ValidationResult,
};
