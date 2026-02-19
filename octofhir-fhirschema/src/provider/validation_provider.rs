use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;

use octofhir_fhir_model::{
    ErrorSeverity, FhirPathConstraint, FhirPathEvaluator, Result as ModelResult,
    ValidationProvider, error::ModelError, provider::ModelProvider,
};

use super::model_provider::FhirSchemaModelProvider;
use crate::embedded::{FhirVersion, create_validation_context, get_schemas};
use crate::terminology::TerminologyService;
use crate::types::ValidationContext;
use octofhir_fhir_model::provider::FhirVersion as ModelFhirVersion;

/// ValidationProvider implementation using FHIR schemas
pub struct FhirSchemaValidationProvider {
    schema_provider: Arc<FhirSchemaModelProvider>,
    #[allow(dead_code)]
    validation_context: ValidationContext,
    /// Optional FHIRPath evaluator for constraint validation
    fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    /// Optional terminology service for binding validation
    terminology_service: Option<Arc<dyn TerminologyService>>,
}

impl FhirSchemaValidationProvider {
    /// Create new validation provider
    pub fn new(
        schema_provider: Arc<FhirSchemaModelProvider>,
        validation_context: ValidationContext,
    ) -> Self {
        Self {
            schema_provider,
            validation_context,
            fhirpath_evaluator: None,
            terminology_service: None,
        }
    }

    /// Add FHIRPath evaluator for constraint validation
    pub fn with_fhirpath_evaluator(mut self, evaluator: Arc<dyn FhirPathEvaluator>) -> Self {
        self.fhirpath_evaluator = Some(evaluator);
        self
    }

    /// Add terminology service for binding validation
    pub fn with_terminology_service(mut self, service: Arc<dyn TerminologyService>) -> Self {
        self.terminology_service = Some(service);
        self
    }

    /// Create validation provider from EmbeddedModelProvider
    pub async fn from_embedded_provider(
        embedded_provider: Arc<dyn ModelProvider>,
        validation_context: ValidationContext,
    ) -> ModelResult<Self> {
        let model_fhir_version = embedded_provider.get_fhir_version().await?;
        let fhir_version = match model_fhir_version {
            ModelFhirVersion::R4 => FhirVersion::R4,
            ModelFhirVersion::R4B => FhirVersion::R4B,
            ModelFhirVersion::R5 => FhirVersion::R5,
            ModelFhirVersion::R6 => FhirVersion::R6,
            ModelFhirVersion::Custom { .. } => FhirVersion::R4, // Default to R4 for custom versions
        };

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            get_schemas(fhir_version).clone(),
            model_fhir_version,
        ));

        Ok(Self {
            schema_provider,
            validation_context,
            fhirpath_evaluator: None,
            terminology_service: None,
        })
    }

    /// Create validation provider from DynamicModelProvider
    pub async fn from_dynamic_provider(
        dynamic_provider: Arc<dyn ModelProvider>,
        validation_context: ValidationContext,
    ) -> ModelResult<Self> {
        let model_fhir_version = dynamic_provider.get_fhir_version().await?;
        let fhir_version = match model_fhir_version {
            ModelFhirVersion::R4 => FhirVersion::R4,
            ModelFhirVersion::R4B => FhirVersion::R4B,
            ModelFhirVersion::R5 => FhirVersion::R5,
            ModelFhirVersion::R6 => FhirVersion::R6,
            ModelFhirVersion::Custom { .. } => FhirVersion::R4, // Default to R4 for custom versions
        };

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            get_schemas(fhir_version).clone(),
            model_fhir_version,
        ));

        Ok(Self {
            schema_provider,
            validation_context,
            fhirpath_evaluator: None,
            terminology_service: None,
        })
    }

    /// Create validation provider with embedded schemas
    pub fn with_embedded_schemas(fhir_version: FhirVersion) -> ModelResult<Self> {
        let schemas = get_schemas(fhir_version);
        let model_fhir_version = match fhir_version {
            FhirVersion::R4 => ModelFhirVersion::R4,
            FhirVersion::R4B => ModelFhirVersion::R4B,
            FhirVersion::R5 => ModelFhirVersion::R5,
            FhirVersion::R6 => ModelFhirVersion::R6,
        };

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            schemas.clone(),
            model_fhir_version,
        ));

        let validation_context = create_validation_context(fhir_version);

        Ok(Self {
            schema_provider,
            validation_context,
            fhirpath_evaluator: None,
            terminology_service: None,
        })
    }

    /// Validate FHIRPath constraints from a schema against a resource
    async fn validate_fhirpath_constraints(
        &self,
        resource: &JsonValue,
        profile_url: &str,
    ) -> ModelResult<bool> {
        // If no evaluator, skip constraint validation (structural validation only)
        let Some(evaluator) = &self.fhirpath_evaluator else {
            return Ok(true);
        };

        // Get schema to extract constraints
        let schema = self
            .schema_provider
            .get_schema_by_url_or_name(profile_url)
            .ok_or_else(|| {
                ModelError::validation_error(format!("Profile not found: {profile_url}"))
            })?;

        // Collect all constraints from the schema
        let mut constraints = Vec::new();

        // Add top-level constraints
        if let Some(schema_constraints) = &schema.constraint {
            for (key, constraint) in schema_constraints {
                constraints.push(
                    FhirPathConstraint::new(
                        key.clone(),
                        constraint.human.clone(),
                        constraint.expression.clone(),
                    )
                    .with_severity(if constraint.severity == "error" {
                        ErrorSeverity::Error
                    } else {
                        ErrorSeverity::Warning
                    }),
                );
            }
        }

        // Add element-level constraints recursively
        if let Some(elements) = &schema.elements {
            Self::collect_element_constraints(elements, &mut constraints);
        }

        if constraints.is_empty() {
            return Ok(true);
        }

        // Validate all constraints using FHIRPath evaluator (Arc avoids deep clone)
        let result = evaluator
            .validate_constraints(Arc::new(resource.clone()), &constraints)
            .await?;

        Ok(result.is_valid)
    }

    /// Recursively collect constraints from element definitions
    fn collect_element_constraints(
        elements: &std::collections::HashMap<String, crate::types::FhirSchemaElement>,
        constraints: &mut Vec<FhirPathConstraint>,
    ) {
        for element in elements.values() {
            if let Some(element_constraints) = &element.constraint {
                for (key, constraint) in element_constraints {
                    constraints.push(
                        FhirPathConstraint::new(
                            key.clone(),
                            constraint.human.clone(),
                            constraint.expression.clone(),
                        )
                        .with_severity(if constraint.severity == "error" {
                            ErrorSeverity::Error
                        } else {
                            ErrorSeverity::Warning
                        }),
                    );
                }
            }

            // Recurse into nested elements
            if let Some(nested) = &element.elements {
                Self::collect_element_constraints(nested, constraints);
            }
        }
    }
}

#[async_trait]
impl ValidationProvider for FhirSchemaValidationProvider {
    async fn validate(&self, resource: &JsonValue, profile_url: &str) -> ModelResult<bool> {
        // Check if profile exists
        let _profile_schema = self
            .schema_provider
            .get_schema_by_url_or_name(profile_url)
            .ok_or_else(|| {
                ModelError::validation_error(format!("Profile not found: {profile_url}"))
            })?;

        // Create FHIR Schema validator with all available schemas
        let mut validator = crate::validation::FhirValidator::from_schemas(
            self.schema_provider.schemas().clone(),
            self.fhirpath_evaluator.clone(),
        );

        // Add terminology service if available
        if let Some(terminology) = &self.terminology_service {
            validator = validator.with_terminology_service(terminology.clone());
        }

        // Validate using the comprehensive FHIR Schema validation engine (async)
        let validation_result = validator
            .validate(resource, vec![profile_url.to_string()])
            .await;

        if !validation_result.valid {
            return Ok(false);
        }

        // Validate FHIRPath constraints if evaluator is available
        let constraints_valid = self
            .validate_fhirpath_constraints(resource, profile_url)
            .await?;

        Ok(constraints_valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FhirSchema;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_validation_provider_creation() {
        let schemas = HashMap::new();
        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            schemas,
            octofhir_fhir_model::FhirVersion::R4,
        ));

        let validation_context = ValidationContext::default();
        let validation_provider =
            FhirSchemaValidationProvider::new(schema_provider, validation_context);

        assert!(validation_provider.schema_provider.schemas().is_empty());
    }

    #[tokio::test]
    async fn test_validation_provider_with_schema() {
        let mut schemas = HashMap::new();
        let test_schema = FhirSchema {
            url: "http://example.org/StructureDefinition/TestProfile".to_string(),
            version: None,
            name: "TestProfile".to_string(),
            type_name: "Patient".to_string(),
            kind: "resource".to_string(),
            derivation: Some("constraint".to_string()),
            base: Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
            abstract_type: None,
            class: "resource".to_string(),
            description: None,
            package_name: None,
            package_version: None,
            package_id: None,
            package_meta: None,
            elements: None,
            required: None,
            excluded: None,
            extensions: None,
            constraint: None,
            primitive_type: None,
            choices: None,
        };

        schemas.insert(
            "http://example.org/StructureDefinition/TestProfile".to_string(),
            test_schema,
        );

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            schemas,
            octofhir_fhir_model::FhirVersion::R4,
        ));

        let validation_context = ValidationContext::default();
        let validation_provider =
            FhirSchemaValidationProvider::new(schema_provider.clone(), validation_context);

        // Test schema availability via ValidationProvider
        use octofhir_fhir_model::ValidationProvider;

        let result = validation_provider
            .validate(
                &json!({"resourceType": "Patient", "id": "test"}),
                "http://example.org/StructureDefinition/TestProfile",
            )
            .await;

        // Should succeed (not crash) whether validation passes or fails
        assert!(
            result.is_ok(),
            "ValidationProvider should handle requests gracefully"
        );
    }
}

/// Create a ValidationProvider from an existing EmbeddedModelProvider
/// This reuses the already initialized provider and its schemas
pub async fn create_validation_provider_from_embedded(
    embedded_provider: Arc<dyn ModelProvider>,
) -> ModelResult<Arc<dyn ValidationProvider>> {
    let model_fhir_version = embedded_provider.get_fhir_version().await?;
    let fhir_version = match model_fhir_version {
        ModelFhirVersion::R4 => FhirVersion::R4,
        ModelFhirVersion::R4B => FhirVersion::R4B,
        ModelFhirVersion::R5 => FhirVersion::R5,
        ModelFhirVersion::R6 => FhirVersion::R6,
        ModelFhirVersion::Custom { .. } => FhirVersion::R4,
    };
    let validation_context = create_validation_context(fhir_version);

    // The EmbeddedModelProvider internally uses FhirSchemaModelProvider with embedded schemas
    // We extract those same schemas to create our ValidationProvider
    let validation_provider =
        FhirSchemaValidationProvider::from_embedded_provider(embedded_provider, validation_context)
            .await?;

    Ok(Arc::new(validation_provider))
}

/// Create a ValidationProvider from an existing DynamicModelProvider
/// This reuses the already initialized provider and its schemas
pub async fn create_validation_provider_from_dynamic(
    dynamic_provider: Arc<dyn ModelProvider>,
) -> ModelResult<Arc<dyn ValidationProvider>> {
    let model_fhir_version = dynamic_provider.get_fhir_version().await?;
    let fhir_version = match model_fhir_version {
        ModelFhirVersion::R4 => FhirVersion::R4,
        ModelFhirVersion::R4B => FhirVersion::R4B,
        ModelFhirVersion::R5 => FhirVersion::R5,
        ModelFhirVersion::R6 => FhirVersion::R6,
        ModelFhirVersion::Custom { .. } => FhirVersion::R4,
    };
    let validation_context = create_validation_context(fhir_version);

    // The DynamicModelProvider internally uses FhirSchemaModelProvider with dynamic schemas
    // We extract those same schemas to create our ValidationProvider
    let validation_provider =
        FhirSchemaValidationProvider::from_dynamic_provider(dynamic_provider, validation_context)
            .await?;

    Ok(Arc::new(validation_provider))
}

/// Create a ValidationProvider with FHIRPath constraint support
///
/// This creates a validation provider that can evaluate FHIRPath constraints
/// in addition to structural schema validation.
pub async fn create_validation_provider_with_fhirpath(
    model_provider: Arc<dyn ModelProvider>,
    fhirpath_evaluator: Arc<dyn FhirPathEvaluator>,
) -> ModelResult<Arc<dyn ValidationProvider>> {
    let model_fhir_version = model_provider.get_fhir_version().await?;
    let fhir_version = match model_fhir_version {
        ModelFhirVersion::R4 => FhirVersion::R4,
        ModelFhirVersion::R4B => FhirVersion::R4B,
        ModelFhirVersion::R5 => FhirVersion::R5,
        ModelFhirVersion::R6 => FhirVersion::R6,
        ModelFhirVersion::Custom { .. } => FhirVersion::R4,
    };
    let validation_context = create_validation_context(fhir_version);

    let validation_provider =
        FhirSchemaValidationProvider::from_embedded_provider(model_provider, validation_context)
            .await?
            .with_fhirpath_evaluator(fhirpath_evaluator);

    Ok(Arc::new(validation_provider))
}
