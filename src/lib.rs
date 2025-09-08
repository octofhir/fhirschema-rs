//! # OctoFHIR FHIRSchema Library
//!
//! A high-performance, async-first Rust library for FHIR schema management and validation.
//!
//! This library provides comprehensive tools for:
//! - Converting FHIR StructureDefinitions to JSON Schema
//! - Validating FHIR resources against schemas
//! - Managing schema caching and storage
//! - Integrating with the broader OctoFHIR ecosystem
//!
//! ## Features
//!
//! - **Async-First**: All operations are async by default using Tokio
//! - **High Performance**: Built for high-throughput FHIR processing
//! - **Type Safety**: Leverages Rust's type system for compile-time guarantees
//! - **Ecosystem Integration**: Seamless interop with OctoFHIR components
//! - **Memory Efficient**: Zero-copy operations where possible
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use octofhir_fhirschema::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//!     // Create a schema manager with default configuration
//!     let config = FhirSchemaConfig::for_version(FhirVersion::R4);
//!     let canonical_manager = octofhir_canonical_manager::CanonicalManager::new(
//!         octofhir_canonical_manager::FcmConfig::default()
//!     ).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
//!     
//!     let manager = FhirSchemaManager::new(config, canonical_manager).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
//!     
//!     // Convert a StructureDefinition to schema
//!     let structure_def = serde_json::json!({
//!         "resourceType": "StructureDefinition",
//!         "url": "http://hl7.org/fhir/StructureDefinition/Patient",
//!         "name": "Patient"
//!         // ... other fields
//!     });
//!     
//!     let result = manager.convert_structure_definition(structure_def).await?;
//!     
//!     if result.success {
//!         println!("Schema converted successfully!");
//!         if let Some(schema) = result.schema {
//!             println!("Schema: {:#?}", schema);
//!         }
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! The library is organized into several key modules:
//!
//! - [`core`]: Core manager and configuration types
//! - [`types`]: Schema type definitions and utilities
//! - [`conversion`]: StructureDefinition to schema conversion
//! - [`validation`]: Resource validation engine
//! - [`storage`]: Schema storage and caching
//! - [`provider`]: ModelProvider implementation for FhirPath integration
//! - [`integration`]: Integration with OctoFHIR ecosystem components

#![allow(missing_docs)]
#![warn(rust_2018_idioms)]
#![forbid(unsafe_code)]

// Public API modules
pub mod conversion;
pub mod core;
pub mod integration;
pub mod provider;
pub mod storage;
pub mod types;
pub mod validation;

// Internal modules
mod error;
mod utils;

// Re-exports for convenience
pub use error::{FhirSchemaError, Result, ValidationError};

/// Convenient prelude for common imports
///
/// This provides everything you need to get started:
///
/// ```rust,no_run
/// use octofhir_fhirschema::prelude::*;
///
/// #[tokio::main]
/// async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
///     // Recommended: Use CompositeModelProvider for best performance
///     let provider = CompositeModelProvider::r4().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
///     
///     // Alternative: Traditional provider (slower startup, full compatibility)
///     // let provider = FhirSchemaModelProvider::r4().await?;
///     
///     // Alternative: Embedded-only provider (fastest startup, limited functionality)
///     // let provider = FhirSchemaModelProvider::embedded_only(FhirVersion::R4).await?;
///     
///     // Get type hierarchy for Patient
///     let hierarchy = provider.get_type_hierarchy("Patient").await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
///     
///     // Navigate a FHIR path
///     let result = provider.navigate_typed_path("Patient", "name.family").await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
///     
///     Ok(())
/// }
/// ```
pub mod prelude {
    // Recommended provider with all optimizations
    pub use crate::provider::CompositeModelProvider;

    // Alternative providers for specific use cases
    pub use crate::provider::FhirSchemaModelProvider;

    #[cfg(feature = "embedded-providers")]
    pub use crate::provider::EmbeddedModelProvider;

    #[cfg(feature = "dynamic-caching")]
    pub use crate::provider::DynamicModelProvider;

    // ModelProvider trait for advanced integrations
    pub use octofhir_fhir_model::provider::ModelProvider;

    // Core types for advanced users
    pub use crate::core::{
        ConversionResult, FhirSchemaConfig, FhirSchemaManager, FhirVersion, ResolutionContext,
        ResolvedType, TypeInfo, ValidationResult,
    };

    // Schema types
    pub use crate::types::{
        ChoiceType, ChoiceTypeResolver, ConstraintSeverity, ElementDefinition, FhirConstraint,
        FhirSchema, FhirSchemaProperty, PathNavigator, TypeHierarchyBuilder, TypeResolver,
    };

    // Provider types for results
    pub use crate::provider::{ChoiceResolution, NavigationResult, TypeHierarchy};

    // Storage types
    pub use crate::storage::{MemoryStorage, SchemaCache, SchemaStorage};

    #[cfg(feature = "dynamic-caching")]
    pub use crate::storage::{CacheStats, DiskStorage, DiskStorageConfig};

    // Error handling
    pub use crate::error::{FhirSchemaError, Result, ValidationError};

    // Utilities
    pub use crate::utils::{PackageFingerprint, generate_package_fingerprint};
}

// Library metadata
/// Current version of the library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Library description
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_metadata() {
        assert_eq!(VERSION, "0.3.0");
        assert_eq!(NAME, "octofhir-fhirschema");
        // DESCRIPTION constant is available from Cargo.toml
    }
}
