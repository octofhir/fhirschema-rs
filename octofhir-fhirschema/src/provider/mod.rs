//! Provider implementations for schema and validation services.
//!
//! This module contains providers for accessing FHIR schemas and performing validation:
//!
//! - **[`model_provider`]** - Schema-based model provider for FHIRPath evaluation
//! - **[`validation_provider`]** - Validation provider for resource validation
//! - **[`builder`]** - Builder pattern for constructing validation providers
//!
//! # Provider Types
//!
//! ## ModelProvider
//!
//! [`FhirSchemaModelProvider`] implements the `ModelProvider` trait for FHIRPath evaluation.
//! It provides type information for navigating FHIR resources during expression evaluation.
//!
//! ```ignore
//! use octofhir_fhirschema::provider::FhirSchemaModelProvider;
//!
//! let provider = FhirSchemaModelProvider::new(schemas, FhirVersion::R4);
//! ```
//!
//! ## ValidationProvider
//!
//! [`FhirSchemaValidationProvider`] implements the `ValidationProvider` trait for resource validation.
//! It validates resources against FHIR schemas and optionally evaluates FHIRPath constraints.
//!
//! ## EmbeddedSchemaProvider / DynamicSchemaProvider
//!
//! - [`EmbeddedSchemaProvider`] - Uses pre-compiled schemas bundled with the crate
//! - [`DynamicSchemaProvider`] - Loads schemas at runtime from StructureDefinitions
//!
//! # Builder Pattern
//!
//! The recommended way to create a validation provider is using [`ValidationProviderBuilder`]:
//!
//! ```ignore
//! use octofhir_fhirschema::provider::ValidationProviderBuilder;
//! use octofhir_fhirschema::embedded::FhirVersion;
//!
//! // Simple validation
//! let provider = ValidationProviderBuilder::new(FhirVersion::R4)
//!     .with_embedded_schemas()
//!     .build()?;
//!
//! // With FHIRPath constraint evaluation
//! let provider = ValidationProviderBuilder::new(FhirVersion::R4)
//!     .with_embedded_schemas()
//!     .with_fhirpath(evaluator)
//!     .build()?;
//! ```
//!
//! # Convenience Functions
//!
//! For quick setup, convenience functions are provided:
//!
//! - [`create_validation_provider_from_embedded`] - Create from embedded schemas
//! - [`create_validation_provider_from_dynamic`] - Create from dynamic schemas
//! - [`create_validation_provider_with_fhirpath`] - Create with FHIRPath support

pub mod builder;
pub mod model_provider;
pub mod validation_provider;

// Re-export main types
pub use builder::ValidationProviderBuilder;
pub use model_provider::{DynamicSchemaProvider, EmbeddedSchemaProvider, FhirSchemaModelProvider};
pub use validation_provider::{
    FhirSchemaValidationProvider, create_validation_provider_from_dynamic,
    create_validation_provider_from_embedded, create_validation_provider_with_fhirpath,
};
