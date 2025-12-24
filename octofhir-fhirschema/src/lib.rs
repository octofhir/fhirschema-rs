//! octofhir-fhirschema - FHIR Schema validation and conversion library.
//!
//! This crate provides:
//! - FHIR Schema validation with FHIRPath constraint support
//! - StructureDefinition to FhirSchema conversion
//! - Model and validation providers for FHIR operations
//!
//! # Quick Start
//!
//! ```ignore
//! use octofhir_fhirschema::{ValidationProviderBuilder, FhirVersion};
//!
//! // Create a validation provider
//! let provider = ValidationProviderBuilder::new(FhirVersion::R4)
//!     .with_embedded_schemas()
//!     .build()?;
//!
//! // Validate a resource
//! let result = provider.validate(&resource, "Patient").await?;
//! ```
//!
//! # Module Organization
//!
//! - [`types`] - Core type definitions (FhirSchema, ValidationError, etc.)
//! - [`provider`] - Schema and validation providers
//! - [`validation`] - Validation engine and error codes
//! - [`embedded`] - Pre-compiled schemas for different FHIR versions
//! - [`converter`] - StructureDefinition to FhirSchema conversion

// Conversion modules
pub mod action_calculator;
pub mod choice_handler;
pub mod converter;
pub mod element_transformer;
pub mod path_parser;
pub mod stack_processor;

// Core modules
pub mod embedded;
pub mod error;
pub mod provider;
pub mod reference;
pub mod terminology;
pub mod types;
pub mod validation;

// Converter exports
pub use converter::translate;

// Embedded schema exports
pub use embedded::{
    FhirVersion, SchemaInfo, create_validation_context, get_schema, get_schema_info,
    get_schema_names, get_schemas, has_schema, list_primitives, list_resources,
};

// Error exports
pub use error::{FhirSchemaError, Result};

// Type exports
pub use types::{
    FhirSchema, FhirSchemaElement, StructureDefinition, ValidationContext, ValidationError,
    ValidationResult,
};

// Validation exports
pub use validation::{FhirSchemaErrorCode, FhirSchemaValidationContext, FhirSchemaValidator};

// Provider exports (from new module structure)
pub use provider::{
    DynamicSchemaProvider, EmbeddedSchemaProvider, FhirSchemaModelProvider,
    FhirSchemaValidationProvider, ValidationProviderBuilder,
    create_validation_provider_from_dynamic, create_validation_provider_from_embedded,
    create_validation_provider_with_fhirpath,
};

// Terminology exports
pub use terminology::{
    BindingStrength, CacheConfig, CacheStats, CachedTerminologyService, CodeValidationResult,
    InMemoryTerminologyService, TerminologyError, TerminologyErrorCode, TerminologyProviderAdapter,
    TerminologyResult, TerminologyService,
};

// Reference validation exports
pub use reference::{
    BundleContext, ContainedContext, NoOpReferenceResolver, ReferenceError, ReferenceErrorCode,
    ReferenceResolutionResult, ReferenceResolver, ReferenceResult,
};

// Re-export key types from fhir-model-rs for convenience
pub use octofhir_fhir_model::error::{ModelError, Result as ModelResult};
pub use octofhir_fhir_model::provider::{
    ElementInfo, FhirVersion as ModelFhirVersion, ModelProvider, TypeInfo,
};

// Re-export terminology types from fhir-model-rs
pub use octofhir_fhir_model::{
    CachedTerminologyProvider, DefaultTerminologyProvider, TerminologyCacheConfig,
    TerminologyCacheStats, TerminologyProvider,
};
