//! Builder pattern for creating ValidationProvider instances.
//!
//! This module provides a fluent builder API for constructing validation providers
//! with various configurations.
//!
//! # Example
//!
//! ```ignore
//! use octofhir_fhirschema::provider::ValidationProviderBuilder;
//! use octofhir_fhirschema::embedded::FhirVersion;
//!
//! // Simple validation with embedded schemas
//! let provider = ValidationProviderBuilder::new(FhirVersion::R4)
//!     .with_embedded_schemas()
//!     .build()?;
//!
//! // Validation with FHIRPath constraint support
//! let provider = ValidationProviderBuilder::new(FhirVersion::R4)
//!     .with_embedded_schemas()
//!     .with_fhirpath(evaluator)
//!     .build()?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use octofhir_fhir_model::{FhirPathEvaluator, Result as ModelResult, error::ModelError};

use super::model_provider::FhirSchemaModelProvider;
use super::validation_provider::FhirSchemaValidationProvider;
use crate::embedded::{FhirVersion, create_validation_context, get_schemas};
use crate::terminology::TerminologyService;
use crate::types::FhirSchema;
use octofhir_fhir_model::provider::FhirVersion as ModelFhirVersion;

/// Builder for creating [`FhirSchemaValidationProvider`] instances.
///
/// Provides a fluent API for configuring validation providers with various options:
/// - FHIR version selection
/// - Embedded or custom schemas
/// - Optional FHIRPath evaluator for constraint validation
/// - Optional terminology service for binding validation
///
/// # Example
///
/// ```ignore
/// let provider = ValidationProviderBuilder::new(FhirVersion::R4)
///     .with_embedded_schemas()
///     .with_fhirpath(evaluator)
///     .with_terminology(terminology_service)
///     .build()?;
/// ```
pub struct ValidationProviderBuilder {
    fhir_version: FhirVersion,
    schemas: Option<HashMap<String, FhirSchema>>,
    fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    terminology_service: Option<Arc<dyn TerminologyService>>,
}

impl ValidationProviderBuilder {
    /// Create a new builder with the specified FHIR version.
    ///
    /// # Arguments
    ///
    /// * `version` - The FHIR version to use (R4, R4B, R5, R6)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = ValidationProviderBuilder::new(FhirVersion::R4);
    /// ```
    pub fn new(version: FhirVersion) -> Self {
        Self {
            fhir_version: version,
            schemas: None,
            fhirpath_evaluator: None,
            terminology_service: None,
        }
    }

    /// Use embedded (pre-compiled) schemas for the specified FHIR version.
    ///
    /// This is the recommended option for most use cases as it provides
    /// fast startup with all standard FHIR types pre-loaded.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = ValidationProviderBuilder::new(FhirVersion::R4)
    ///     .with_embedded_schemas()
    ///     .build()?;
    /// ```
    pub fn with_embedded_schemas(mut self) -> Self {
        self.schemas = Some(get_schemas(self.fhir_version).clone());
        self
    }

    /// Use custom schemas instead of embedded schemas.
    ///
    /// This is useful when you need to:
    /// - Validate against custom profiles
    /// - Use schemas loaded from a different source
    /// - Add additional profiles to the validation context
    ///
    /// # Arguments
    ///
    /// * `schemas` - HashMap of schema name/URL to FhirSchema
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut schemas = get_schemas(FhirVersion::R4).clone();
    /// // Add custom profile
    /// schemas.insert(my_profile_url.clone(), my_profile);
    ///
    /// let provider = ValidationProviderBuilder::new(FhirVersion::R4)
    ///     .with_schemas(schemas)
    ///     .build()?;
    /// ```
    pub fn with_schemas(mut self, schemas: HashMap<String, FhirSchema>) -> Self {
        self.schemas = Some(schemas);
        self
    }

    /// Add a FHIRPath evaluator for constraint validation.
    ///
    /// When a FHIRPath evaluator is provided, the validator will evaluate
    /// FHIRPath constraint expressions in addition to structural validation.
    ///
    /// Without a FHIRPath evaluator, only structural validation is performed
    /// (type checking, cardinality, required elements, etc.).
    ///
    /// # Arguments
    ///
    /// * `evaluator` - An Arc-wrapped FHIRPath evaluator implementation
    ///
    /// # Example
    ///
    /// ```ignore
    /// use octofhir_fhirpath::FhirPathEvaluator;
    ///
    /// let fhirpath = Arc::new(FhirPathEvaluator::new(model_provider));
    /// let provider = ValidationProviderBuilder::new(FhirVersion::R4)
    ///     .with_embedded_schemas()
    ///     .with_fhirpath(fhirpath)
    ///     .build()?;
    /// ```
    pub fn with_fhirpath(mut self, evaluator: Arc<dyn FhirPathEvaluator>) -> Self {
        self.fhirpath_evaluator = Some(evaluator);
        self
    }

    /// Add a terminology service for binding validation.
    ///
    /// When a terminology service is provided, the validator will validate
    /// coded elements against their bound value sets based on binding strength:
    /// - `required`: Code MUST be from the value set (error if not)
    /// - `extensible`: Code SHOULD be from the value set (warning if not)
    /// - `preferred`: Code recommended from the value set (informational)
    /// - `example`: No validation performed
    ///
    /// Without a terminology service, binding validation is skipped.
    ///
    /// # Arguments
    ///
    /// * `service` - An Arc-wrapped terminology service implementation
    ///
    /// # Example
    ///
    /// ```ignore
    /// use octofhir_fhirschema::terminology::{CachedTerminologyService, CacheConfig};
    ///
    /// let terminology = Arc::new(CachedTerminologyService::new(
    ///     inner_service,
    ///     CacheConfig::default(),
    /// ));
    /// let provider = ValidationProviderBuilder::new(FhirVersion::R4)
    ///     .with_embedded_schemas()
    ///     .with_terminology(terminology)
    ///     .build()?;
    /// ```
    pub fn with_terminology(mut self, service: Arc<dyn TerminologyService>) -> Self {
        self.terminology_service = Some(service);
        self
    }

    /// Build the validation provider.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No schemas were provided (call `with_embedded_schemas()` or `with_schemas()` first)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = ValidationProviderBuilder::new(FhirVersion::R4)
    ///     .with_embedded_schemas()
    ///     .build()?;
    /// ```
    pub fn build(self) -> ModelResult<FhirSchemaValidationProvider> {
        let schemas = self.schemas.ok_or_else(|| {
            ModelError::schema_load_error(
                "No schemas provided. Call with_embedded_schemas() or with_schemas() before build()"
            )
        })?;

        let model_fhir_version = match self.fhir_version {
            FhirVersion::R4 => ModelFhirVersion::R4,
            FhirVersion::R4B => ModelFhirVersion::R4B,
            FhirVersion::R5 => ModelFhirVersion::R5,
            FhirVersion::R6 => ModelFhirVersion::R6,
        };

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(schemas, model_fhir_version));

        let validation_context = create_validation_context(self.fhir_version);

        let mut provider = FhirSchemaValidationProvider::new(schema_provider, validation_context);

        if let Some(evaluator) = self.fhirpath_evaluator {
            provider = provider.with_fhirpath_evaluator(evaluator);
        }

        if let Some(terminology) = self.terminology_service {
            provider = provider.with_terminology_service(terminology);
        }

        Ok(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_requires_schemas() {
        let result = ValidationProviderBuilder::new(FhirVersion::R4).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_embedded_schemas() {
        let result = ValidationProviderBuilder::new(FhirVersion::R4)
            .with_embedded_schemas()
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_with_custom_schemas() {
        let schemas = HashMap::new();
        let result = ValidationProviderBuilder::new(FhirVersion::R4)
            .with_schemas(schemas)
            .build();
        assert!(result.is_ok());
    }
}
